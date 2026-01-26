use super::GrancClient;
use crate::grpc::client::GrpcRequestError;
use crate::{
    BoxError,
    client::model::{Descriptor, DynamicRequest, DynamicResponse},
    grpc::client::GrpcClient,
};
use futures_util::Stream;
use http_body::Body as HttpBody;
use prost_reflect::DescriptorPool;
use tokio_stream::StreamExt;

#[derive(Debug, thiserror::Error)]
pub enum DynamicCallError {
    #[error("Invalid input: '{0}'")]
    InvalidInput(String),

    #[error("Service '{0}' not found")]
    ServiceNotFound(String),

    #[error("Method '{0}' not found")]
    MethodNotFound(String),

    #[error("gRPC client request error: '{0}'")]
    GrpcRequestError(#[from] GrpcRequestError),
}

pub struct WithFileDescriptor<S> {
    grpc_client: GrpcClient<S>,
    pool: DescriptorPool,
}

impl<S> GrancClient<WithFileDescriptor<S>> {
    pub(crate) fn new(grpc_client: GrpcClient<S>, pool: DescriptorPool) -> Self {
        Self {
            state: WithFileDescriptor {
                grpc_client,
                pool: pool,
            },
        }
    }
}

impl<S> GrancClient<WithFileDescriptor<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    pub fn list_services(&mut self) -> Vec<String> {
        self.state
            .pool
            .services()
            .map(|s| s.full_name().to_string())
            .collect()
    }

    pub fn get_descriptor_by_symbol(&mut self, symbol: &str) -> Option<Descriptor> {
        let pool = &self.state.pool;

        if let Some(descriptor) = pool.get_service_by_name(symbol) {
            return Some(Descriptor::ServiceDescriptor(descriptor));
        }

        if let Some(descriptor) = pool.get_message_by_name(symbol) {
            return Some(Descriptor::MessageDescriptor(descriptor));
        }

        if let Some(descriptor) = pool.get_enum_by_name(symbol) {
            return Some(Descriptor::EnumDescriptor(descriptor));
        }

        None
    }

    pub async fn dynamic(
        &mut self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicCallError> {
        let method = self
            .state
            .pool
            .get_service_by_name(&request.service)
            .ok_or_else(|| DynamicCallError::ServiceNotFound(request.service))?
            .methods()
            .find(|m| m.name() == request.method)
            .ok_or_else(|| DynamicCallError::MethodNotFound(request.method))?;

        match (method.is_client_streaming(), method.is_server_streaming()) {
            (false, false) => {
                let result = self
                    .state
                    .grpc_client
                    .unary(method, request.body, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }

            (false, true) => {
                match self
                    .state
                    .grpc_client
                    .server_streaming(method, request.body, request.headers)
                    .await?
                {
                    Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                    Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
                }
            }
            (true, false) => {
                let input_stream =
                    json_array_to_stream(request.body).map_err(DynamicCallError::InvalidInput)?;
                let result = self
                    .state
                    .grpc_client
                    .client_streaming(method, input_stream, request.headers)
                    .await?;
                Ok(DynamicResponse::Unary(result))
            }

            (true, true) => {
                let input_stream =
                    json_array_to_stream(request.body).map_err(DynamicCallError::InvalidInput)?;
                match self
                    .state
                    .grpc_client
                    .bidirectional_streaming(method, input_stream, request.headers)
                    .await?
                {
                    Ok(stream) => Ok(DynamicResponse::Streaming(Ok(stream.collect().await))),
                    Err(status) => Ok(DynamicResponse::Streaming(Err(status))),
                }
            }
        }
    }
}

/// Helper to convert a JSON Array into a Stream of JSON Values.
/// Required for Client and Bidirectional streaming.
fn json_array_to_stream(
    json: serde_json::Value,
) -> Result<impl Stream<Item = serde_json::Value> + Send + 'static, String> {
    match json {
        serde_json::Value::Array(items) => Ok(tokio_stream::iter(items)),
        _ => Err("Client streaming requires a JSON Array body".to_string()),
    }
}
