//! # Errors
//!
//! ## Purpose
//! Defines the domain-specific error types for the gateway and utilities for mapping
//! gRPC status codes to HTTP status codes. This module ensures that internal errors
//! and upstream gRPC errors are correctly translated into HTTP responses.
//!
//! ## Scope
//! This module defines:
//! -   `GatewayError`: The primary error type representing failures within the gateway or upstream interactions.
//! -   `map_code_to_status`: A utility to convert gRPC `Code`s to HTTP `StatusCode`s.
//! -   `handle_error`: A utility to construct a JSON error response from a `tonic::Status`.
//!
//! ## Position in the Architecture
//! Errors originating from the codec, router, or upstream gRPC calls are captured as `GatewayError`.
//! The generated code uses `handle_error` to transform these errors into standard HTTP responses
//! before sending them to the client.
//!
//! ## Design Constraints
//! -   **Dual-Mode Error Handling**: When `std` is enabled, it leverages `thiserror` for ergonomic error definition.
//!     In `no_std` environments, it falls back to a manual implementation using `alloc`.

#[allow(unused_imports)]
use crate::alloc;
use crate::BoxBody;
use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::{BodyExt, Full};
use tonic::Code;

#[cfg(feature = "std")]
use thiserror::Error;

/// Domain-specific errors for the gateway.
///
/// This enum encapsulates various failure modes including serialization issues,
/// upstream gRPC errors, and HTTP protocol violations.
#[cfg(feature = "std")]
#[derive(Debug, Error)]
pub enum GatewayError {
    /// Represents a failure during message serialization or deserialization.
    #[error("failed to serialize/deserialize message: {0}")]
    Encoding(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// Represents an error returned by the upstream gRPC service.
    #[error("upstream gRPC error: {0}")]
    Upstream(#[from] tonic::Status),

    /// Represents an error within the HTTP protocol handling (e.g., building a response).
    #[error("HTTP protocol error: {0}")]
    Http(#[from] http::Error),

    /// Indicates that the requested HTTP method is not allowed for the path.
    #[error("Method not allowed")]
    MethodNotAllowed,

    /// Indicates that no route matched the request path.
    #[error("Not found")]
    NotFound,
}

#[cfg(not(feature = "std"))]
#[derive(Debug)]
pub enum GatewayError {
    Encoding(alloc::string::String),
    Upstream(tonic::Status),
    Http(http::Error),
    MethodNotAllowed,
    NotFound,
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GatewayError::Encoding(e) => {
                write!(f, "failed to serialize/deserialize message: {}", e)
            }
            GatewayError::Upstream(s) => write!(f, "upstream gRPC error: {}", s),
            GatewayError::Http(e) => write!(f, "HTTP protocol error: {}", e),
            GatewayError::MethodNotAllowed => write!(f, "Method not allowed"),
            GatewayError::NotFound => write!(f, "Not found"),
        }
    }
}

/// Maps a gRPC status code to an HTTP status code.
///
/// Adheres to the canonical mapping defined in `google.rpc.Code`.
///
/// # Parameters
/// *   `code`: The gRPC status code.
///
/// # Returns
/// The corresponding `http::StatusCode`.
pub fn map_code_to_status(code: Code) -> StatusCode {
    match code {
        Code::Ok => StatusCode::OK,
        Code::Cancelled => StatusCode::from_u16(499).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Code::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        Code::InvalidArgument => StatusCode::BAD_REQUEST,
        Code::DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
        Code::NotFound => StatusCode::NOT_FOUND,
        Code::AlreadyExists => StatusCode::CONFLICT,
        Code::PermissionDenied => StatusCode::FORBIDDEN,
        Code::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
        Code::FailedPrecondition => StatusCode::BAD_REQUEST,
        Code::Aborted => StatusCode::CONFLICT,
        Code::OutOfRange => StatusCode::BAD_REQUEST,
        Code::Unimplemented => StatusCode::NOT_IMPLEMENTED,
        Code::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        Code::DataLoss => StatusCode::INTERNAL_SERVER_ERROR,
        Code::Unauthenticated => StatusCode::UNAUTHORIZED,
    }
}

/// Converts a gRPC status into an HTTP response.
///
/// This helper creates a JSON response body containing the error code and message,
/// falling back to standard HTTP status codes derived from the gRPC status.
///
/// # Parameters
/// *   `status`: The `tonic::Status` to convert.
///
/// # Returns
/// An `http::Response` containing the JSON-encoded error details.
pub fn handle_error(status: tonic::Status) -> Response<BoxBody> {
    let http_code = map_code_to_status(status.code());

    // Fallback to JSON error response as per grpc-gateway default behavior
    let body = serde_json::json!({
        "code": status.code() as i32,
        "message": status.message(),
        "details": []
    });

    let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
    let full_body =
        BodyExt::boxed_unsync(Full::new(Bytes::from(body_bytes)).map_err(|never| match never {}));

    Response::builder()
        .status(http_code)
        .header("Content-Type", "application/json")
        .body(full_body)
        .unwrap_or_else(|_| {
            Response::new(BodyExt::boxed_unsync(
                Full::new(Bytes::new()).map_err(|never| match never {}),
            ))
        })
}
