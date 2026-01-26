use super::{
    Descriptor, DynamicRequest, DynamicResponse, GrancClient,
    with_file_descriptor::WithFileDescriptor,
};
use crate::{
    BoxError,
    grpc::client::GrpcClient,
    reflection::client::{ReflectionClient, ReflectionResolveError},
};
use http_body::Body as HttpBody;
use prost_reflect::DescriptorError;
use prost_reflect::DescriptorPool;
use std::fmt::Debug;
use tonic::{
    Code,
    transport::{Channel, Endpoint},
};

#[derive(Debug, thiserror::Error)]
pub enum ClientConnectError {
    #[error("Invalid URL '{0}': {1}")]
    InvalidUrl(String, #[source] tonic::transport::Error),
    #[error("Failed to connect to '{0}': {1}")]
    ConnectionFailed(String, #[source] tonic::transport::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicCallError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),

    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),

    #[error(transparent)]
    DynamicCallError(#[from] super::with_file_descriptor::DynamicCallError),
}

#[derive(Debug, thiserror::Error)]
pub enum GetDescriptorError {
    #[error("Reflection resolution failed: '{0}'")]
    ReflectionResolve(#[from] ReflectionResolveError),
    #[error("Failed to decode file descriptor set: '{0}'")]
    DescriptorError(#[from] DescriptorError),
    #[error("Descriptor at path '{0}' not found")]
    NotFound(String),
}

#[derive(Debug, Clone)]
pub struct WithServerReflection<S> {
    reflection_client: ReflectionClient<S>,
    grpc_client: GrpcClient<S>,
}

impl GrancClient<WithServerReflection<Channel>> {
    /// Connects to a gRPC server at the specified address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The URI of the server (e.g., `http://localhost:50051`).
    ///
    /// # Errors
    ///
    /// Returns a [`ClientConnectError`] if the URL is invalid or the connection cannot be established.
    pub async fn connect(addr: &str) -> Result<Self, ClientConnectError> {
        let endpoint = Endpoint::new(addr.to_string())
            .map_err(|e| ClientConnectError::InvalidUrl(addr.to_string(), e))?;

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| ClientConnectError::ConnectionFailed(addr.to_string(), e))?;

        Ok(Self::from_service(channel))
    }
}

impl<S> GrancClient<WithServerReflection<S>>
where
    S: tonic::client::GrpcService<tonic::body::Body> + Clone,
    S::ResponseBody: HttpBody<Data = tonic::codegen::Bytes> + Send + 'static,
    <S::ResponseBody as HttpBody>::Error: Into<BoxError> + Send,
{
    /// Creates a new `GrancClient` wrapping an existing Tonic service (e.g., a `Channel`).
    pub fn from_service(service: S) -> Self {
        let reflection_client = ReflectionClient::new(service.clone());
        let grpc_client = GrpcClient::new(service);

        Self {
            state: WithServerReflection {
                reflection_client,
                grpc_client,
            },
        }
    }

    pub fn with_file_descriptor(
        self,
        file_descriptor: Vec<u8>,
    ) -> Result<GrancClient<WithFileDescriptor<S>>, DescriptorError> {
        let pool = DescriptorPool::decode(file_descriptor.as_slice())?;
        Ok(GrancClient::new(self.state.grpc_client, pool))
    }

    pub async fn list_services(&mut self) -> Result<Vec<String>, ReflectionResolveError> {
        self.state.reflection_client.list_services().await
    }

    pub async fn get_descriptor_by_symbol(
        &mut self,
        symbol: &str,
    ) -> Result<Descriptor, GetDescriptorError> {
        let fd_set = self
            .state
            .reflection_client
            .file_descriptor_set_by_symbol(symbol)
            .await
            .map_err(|err| match err {
                ReflectionResolveError::ServerStreamFailure(status)
                    if status.code() == Code::NotFound =>
                {
                    GetDescriptorError::NotFound(symbol.to_string())
                }
                err => GetDescriptorError::ReflectionResolve(err),
            })?;

        let pool = DescriptorPool::from_file_descriptor_set(fd_set)?;

        let mut client =
            GrancClient::<WithFileDescriptor<S>>::new(self.state.grpc_client.clone(), pool);

        client
            .get_descriptor_by_symbol(symbol)
            .ok_or_else(|| GetDescriptorError::NotFound(symbol.to_string()))
    }

    pub async fn dynamic(
        &mut self,
        request: DynamicRequest,
    ) -> Result<DynamicResponse, DynamicCallError> {
        let fd_set = self
            .state
            .reflection_client
            .file_descriptor_set_by_symbol(&request.service)
            .await?;

        let pool = DescriptorPool::from_file_descriptor_set(fd_set)?;

        let mut client =
            GrancClient::<WithFileDescriptor<S>>::new(self.state.grpc_client.clone(), pool);

        Ok(client.dynamic(request).await?)
    }
}
