use crate::config::{ConfigManager, ConnectionConfig, Project};
use crate::globals;
use crate::model::{MethodData, Model};
use crate::msg::Msg;
use color_eyre::Result;
use granc_core::client::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};

#[derive(Debug, Clone)]
pub enum Effect {
    LoadConfigFromDisk,
    SaveConfigToDisk(crate::config::AppConfig),
    FetchServices(Project),
    FetchMethods {
        project: Project,
        service: String,
    },
    ExecuteCall {
        project: Project,
        service: String,
        method: String,
        body: String,
        headers: Vec<(String, String)>,
    },
}

pub fn handle_effect(_model: &Model, effect: Effect) -> Result<Option<Msg>> {
    match effect {
        Effect::LoadConfigFromDisk => {
            let manager = ConfigManager::new().map_err(|e| color_eyre::eyre::eyre!(e))?;
            match manager.load() {
                Ok(cfg) => Ok(Some(Msg::ConfigLoaded(Ok(cfg)))),
                Err(e) => Ok(Some(Msg::ConfigLoaded(Err(e.to_string())))),
            }
        }

        Effect::SaveConfigToDisk(cfg) => {
            if let Ok(manager) = ConfigManager::new() {
                let _ = manager.save(&cfg);
            }
            Ok(None)
        }

        Effect::FetchServices(proj) => {
            let handle = globals::get_handle();
            let project_id = proj.id;
            let result = handle.block_on(async move { fetch_services_async(&proj).await });

            match result {
                Ok(services) => Ok(Some(Msg::ServicesFetched {
                    project_id,
                    services,
                })),
                Err(e) => Ok(Some(Msg::CallResponse(Err(format!("Fetch failed: {}", e))))),
            }
        }

        Effect::FetchMethods { project, service } => {
            let handle = globals::get_handle();
            let service_name = service.clone();
            let result =
                handle.block_on(async move { fetch_methods_async(&project, &service_name).await });

            match result {
                Ok(methods) => Ok(Some(Msg::MethodsFetched { service, methods })),
                Err(e) => Ok(Some(Msg::CallResponse(Err(format!(
                    "Fetch methods failed: {}",
                    e
                ))))),
            }
        }

        Effect::ExecuteCall {
            project,
            service,
            method,
            body,
            headers,
        } => {
            let handle = globals::get_handle();
            let result = handle.block_on(async move {
                execute_call_async(&project, service, method, body, headers).await
            });

            Ok(Some(Msg::CallResponse(result)))
        }
    }
}

// --- Async Handlers ---

async fn fetch_services_async(proj: &Project) -> std::result::Result<Vec<String>, String> {
    match &proj.connection {
        ConnectionConfig::Reflection { url } => {
            let mut client = GrancClient::connect(url).await.map_err(|e| e.to_string())?;
            client.list_services().await.map_err(|e| e.to_string())
        }
        ConnectionConfig::File { url, path } => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            if let Ok(c) = GrancClient::connect(url).await {
                if let Ok(fc) = c.with_file_descriptor(bytes.clone()) {
                    return Ok(fc.list_services());
                }
            }
            let client = GrancClient::offline(bytes).map_err(|e| e.to_string())?;
            Ok(client.list_services())
        }
    }
}

async fn fetch_methods_async(
    proj: &Project,
    service: &str,
) -> std::result::Result<Vec<MethodData>, String> {
    fn extract(descriptor: Descriptor) -> std::result::Result<Vec<MethodData>, String> {
        match descriptor {
            Descriptor::ServiceDescriptor(sd) => {
                let methods = sd
                    .methods()
                    .map(|m| {
                        let input_desc = m.input();
                        let input = input_desc.name();
                        let output_desc = m.output();
                        let output = output_desc.name();
                        let client_stream = if m.is_client_streaming() {
                            "stream "
                        } else {
                            ""
                        };
                        let server_stream = if m.is_server_streaming() {
                            "stream "
                        } else {
                            ""
                        };

                        MethodData {
                            name: m.name().to_string(),
                            signature: format!(
                                "rpc {}({}{}) returns ({}{})",
                                m.name(),
                                client_stream,
                                input,
                                server_stream,
                                output
                            ),
                        }
                    })
                    .collect();
                Ok(methods)
            }
            _ => Err("Symbol is not a service".to_string()),
        }
    }

    match &proj.connection {
        ConnectionConfig::Reflection { url } => {
            let mut client = GrancClient::connect(url).await.map_err(|e| e.to_string())?;
            let descriptor = client
                .get_descriptor_by_symbol(service)
                .await
                .map_err(|e| e.to_string())?;
            extract(descriptor)
        }
        ConnectionConfig::File { url, path } => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            if let Ok(c) = GrancClient::connect(url).await {
                // Fixed: Removed 'mut' from fc
                if let Ok(fc) = c.with_file_descriptor(bytes.clone()) {
                    if let Some(d) = fc.get_descriptor_by_symbol(service) {
                        return extract(d);
                    }
                }
            }
            let client = GrancClient::offline(bytes).map_err(|e| e.to_string())?;
            if let Some(d) = client.get_descriptor_by_symbol(service) {
                return extract(d);
            }
            Err("Service not found".to_string())
        }
    }
}

async fn execute_call_async(
    proj: &Project,
    service: String,
    method: String,
    body: String,
    headers: Vec<(String, String)>,
) -> std::result::Result<String, String> {
    let json_body: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Invalid JSON: {}", e))?;

    let req = DynamicRequest {
        service,
        method,
        body: json_body,
        headers,
    };

    match &proj.connection {
        ConnectionConfig::Reflection { url } => {
            let mut client = GrancClient::connect(url).await.map_err(|e| e.to_string())?;
            let resp = client.dynamic(req).await.map_err(|e| e.to_string())?;
            format_response(resp)
        }
        ConnectionConfig::File { url, path } => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let client = GrancClient::connect(url).await.map_err(|e| e.to_string())?;
            let mut client = client
                .with_file_descriptor(bytes)
                .map_err(|e| e.to_string())?;
            let resp = client.dynamic(req).await.map_err(|e| e.to_string())?;
            format_response(resp)
        }
    }
}

fn format_response(resp: DynamicResponse) -> std::result::Result<String, String> {
    match resp {
        DynamicResponse::Unary(Ok(v)) => Ok(serde_json::to_string_pretty(&v).unwrap_or_default()),
        DynamicResponse::Unary(Err(s)) => {
            Err(format!("gRPC Error: {} (Code: {})", s.message(), s.code()))
        }
        DynamicResponse::Streaming(r) => {
            let mut out = String::new();
            match r {
                Ok(msgs) => {
                    for (i, msg) in msgs.into_iter().enumerate() {
                        match msg {
                            Ok(v) => out.push_str(&format!(
                                "Msg {}:\n{}\n",
                                i,
                                serde_json::to_string_pretty(&v).unwrap_or_default()
                            )),
                            Err(s) => out.push_str(&format!("Msg {} Error: {}\n", i, s.message())),
                        }
                    }
                    Ok(out)
                }
                Err(s) => Err(format!("Stream Error: {}", s.message())),
            }
        }
    }
}
