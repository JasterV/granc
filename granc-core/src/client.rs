//! # Granc Client
//!
//! This module implements the high-level logic for executing dynamic gRPC requests
//! and offers support for reflection operations if the server supports it.
//!
//! The [`GrancClient`] is the primary entry point for consumers of this library.
//! It abstracts away the complexity of connection management, schema resolution (reflection vs. file descriptors),
//! and generic gRPC transport.
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use granc_core::client::{GrancClient, DynamicRequest};
//! use serde_json::json;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Connect to the server
//! let mut client = GrancClient::connect("http://localhost:50051").await?;
//!
//! // 2. Prepare the request (using server reflection)
//! let request = DynamicRequest {
//!     service: "helloworld.Greeter".to_string(),
//!     method: "SayHello".to_string(),
//!     body: json!({ "name": "Ferris" }),
//!     headers: vec![],
//!     file_descriptor_set: None,
//! };
//!
//! // 3. Execute the call
//! let response = client.dynamic(request).await?;
//! println!("Response: {:?}", response);
//! # Ok(())
//! # }
//! ```

mod model;
pub mod with_file_descriptor;
pub mod with_server_reflection;

pub use model::{Descriptor, DynamicRequest, DynamicResponse};

pub struct GrancClient<T: locked::Locked> {
    state: T,
}

mod locked {
    use crate::client::{
        with_file_descriptor::WithFileDescriptor, with_server_reflection::WithServerReflection,
    };

    pub trait Locked {}

    impl<S> Locked for WithFileDescriptor<S> {}
    impl<S> Locked for WithServerReflection<S> {}
}
