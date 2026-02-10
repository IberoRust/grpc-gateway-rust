//! # Gateway Codegen
//!
//! ## Purpose
//! This crate implements the core logic for the `protoc-gen-grpc-gateway` plugin.
//! It transforms Protocol Buffers descriptors (parsed from `CodeGeneratorRequest`) into
//! Rust source code that implements a gRPC-Gateway using `gateway-runtime`.
//!
//! ## Scope
//! This library handles the entire code generation pipeline:
//! -   **Validation**: Checks input `.proto` files for supported features and validity.
//! -   **Processing**: Extracts service and method definitions, parsing HTTP rules and annotations.
//! -   **Path Compilation**: Compiles HTTP path templates (e.g., `/v1/{name=messages/*}`) into efficient matching opcodes.
//! -   **Generation**: Produces Rust ASTs (Abstract Syntax Trees) and token streams for the final output.
//!
//! ## Architecture
//! The data flow through this crate is as follows:
//! 1.  `main.rs` (in the binary) decodes the `CodeGeneratorRequest`.
//! 2.  [`validator`] validates the input descriptors.
//! 3.  [`descriptor_processor`] iterates through services and methods, building an intermediate representation (`ServiceDefinition`).
//! 4.  [`path_compiler`] compiles the path patterns found in `google.api.http` options.
//! 5.  [`generator`] takes the `ServiceDefinition` and produces the final Rust code using `quote`.
//!
//! ## Usage
//! This crate is primarily intended to be used by the `protoc-gen-grpc-gateway` binary.
//! However, it can be imported for testing or advanced integration scenarios.

#![doc(html_root_url = "https://docs.rs/protoc-gen-grpc-gateway/0.2.1")]

/// Processes Protobuf descriptors into an intermediate service representation.
pub mod descriptor_processor;

/// Generates Rust source code from processed service definitions.
pub mod generator;

/// Compiles HTTP path templates into matching opcodes.
pub mod path_compiler;

/// Validates Protobuf descriptors before processing.
pub mod validator;
