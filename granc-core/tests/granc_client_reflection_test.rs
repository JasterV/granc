use echo_service::{EchoServiceServer, FILE_DESCRIPTOR_SET};
use echo_service_impl::EchoServiceImpl;
use granc_core::client::{Descriptor, DynamicRequest, DynamicResponse, GrancClient};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

mod echo_service_impl;

async fn spawn_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
            .build_v1()
            .unwrap();

        let echo_service = EchoServiceServer::new(EchoServiceImpl);

        Server::builder()
            .add_service(reflection_service)
            .add_service(echo_service)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    addr
}

#[tokio::test]
async fn test_reflection_list_services() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    let services = client.list_services().await.unwrap();
    assert!(services.contains(&"echo.EchoService".to_string()));
    assert!(services.contains(&"grpc.reflection.v1.ServerReflection".to_string()));
}

#[tokio::test]
async fn test_reflection_describe_descriptors() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // 1. Describe Service
    let desc = client
        .get_descriptor_by_symbol("echo.EchoService")
        .await
        .unwrap();
    if let Descriptor::ServiceDescriptor(s) = desc {
        assert_eq!(s.name(), "EchoService");
        assert!(s.methods().any(|m| m.name() == "UnaryEcho"));
    } else {
        panic!("Expected ServiceDescriptor");
    }

    // 2. Describe Message
    let desc = client
        .get_descriptor_by_symbol("echo.EchoRequest")
        .await
        .unwrap();
    if let Descriptor::MessageDescriptor(m) = desc {
        assert_eq!(m.name(), "EchoRequest");
        assert!(m.fields().any(|f| f.name() == "message"));
    } else {
        panic!("Expected MessageDescriptor");
    }

    // 3. Error Case: Non-existent symbol
    let err = client.get_descriptor_by_symbol("echo.Ghost").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn test_reflection_dynamic_calls() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // 1. Unary Call
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({ "message": "hello" }),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Unary(Ok(val)) = res {
        assert_eq!(val["message"], "hello");
    } else {
        panic!("Unexpected response type for Unary");
    }

    // 2. Server Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ServerStreamingEcho".to_string(),
        body: serde_json::json!({ "message": "stream" }),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Streaming(Ok(stream)) = res {
        assert_eq!(stream.len(), 3);
    } else {
        panic!("Unexpected response type for Server Streaming");
    }

    // 3. Client Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "ClientStreamingEcho".to_string(),
        body: serde_json::json!([
            { "message": "A" },
            { "message": "B" }
        ]),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Unary(Ok(val)) = res {
        assert_eq!(val["message"], "AB");
    } else {
        panic!("Unexpected response type for Client Streaming");
    }

    // 4. Bidirectional Streaming
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "BidirectionalEcho".to_string(),
        body: serde_json::json!([
            { "message": "Ping" }
        ]),
        headers: vec![],
    };
    let res = client.dynamic(req).await.unwrap();
    if let DynamicResponse::Streaming(Ok(stream)) = res {
        assert_eq!(stream.len(), 1);
        assert_eq!(stream[0].as_ref().unwrap()["message"], "echo: Ping");
    } else {
        panic!("Unexpected response type for Bidi Streaming");
    }
}

#[tokio::test]
async fn test_reflection_error_cases() {
    let addr = spawn_server().await;
    let url = format!("http://{}", addr);
    let mut client = GrancClient::connect(&url).await.unwrap();

    // 1. Invalid Service Name
    let req = DynamicRequest {
        service: "echo.GhostService".to_string(),
        method: "UnaryEcho".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };
    assert!(client.dynamic(req).await.is_err());

    // 2. Invalid Method Name
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "GhostMethod".to_string(),
        body: serde_json::json!({}),
        headers: vec![],
    };
    assert!(client.dynamic(req).await.is_err());

    // 3. Invalid JSON Body
    let req = DynamicRequest {
        service: "echo.EchoService".to_string(),
        method: "UnaryEcho".to_string(),
        // Field "wrong" does not exist in EchoRequest
        body: serde_json::json!({ "wrong": "field" }),
        headers: vec![],
    };
    // This might fail at encoding time
    let response = client.dynamic(req).await;

    println!("{response:#?}");

    assert!(response.is_err())
}
