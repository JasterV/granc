//! # Granc CLI Entry Point
//!
//! The main executable for the Granc tool. This file drives the application lifecycle:
//!
//! 1. **Initialization**: Parses command-line arguments using [`cli::Cli`].
//! 2. **Connection**: Establishes a TCP connection to the target server via `granc_core`.
//! 3. **Execution**: Delegates the request processing to the `GrancClient`.
//! 4. **Presentation**: Formats and prints the resulting JSON or error status to standard output/error.

mod cli;

use clap::Parser;
use cli::Cli;
use granc_core::client::{DynamicRequest, DynamicResponse, GrancClient};
use std::process;

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let file_descriptor_set = match args
        .file_descriptor_set
        .map(|path| std::fs::read(path))
        .transpose()
    {
        Ok(fd) => fd,
        Err(err) => {
            eprintln!("Error reading file descriptor set: '{err}'");
            process::exit(1);
        }
    };

    let (service, method) = args.endpoint;

    let request = DynamicRequest {
        file_descriptor_set,
        body: args.body,
        headers: args.headers,
        service,
        method,
    };

    let mut client = match GrancClient::connect(&args.url).await {
        Ok(client) => client,
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    };

    match client.dynamic(DynamicRequest::from(request)).await {
        Ok(DynamicResponse::Unary(Ok(value))) => print_json(&value),
        Ok(DynamicResponse::Unary(Err(status))) => print_status(&status),
        Ok(DynamicResponse::Streaming(Ok(values))) => print_stream(&values),
        Ok(DynamicResponse::Streaming(Err(status))) => print_status(&status),
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    }
}

fn print_json(val: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string())
    );
}

fn print_status(status: &tonic::Status) {
    eprintln!(
        "gRPC Failed: code={:?} message={:?}",
        status.code(),
        status.message()
    );
}

fn print_stream(stream: &[Result<serde_json::Value, tonic::Status>]) {
    for elem in stream {
        match elem {
            Ok(val) => print_json(val),
            Err(status) => print_status(status),
        }
    }
}
