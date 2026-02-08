//! # Metadata
//!
//! ## Purpose
//! Utilities for translating HTTP headers into gRPC metadata. This allows context
//! such as authentication tokens, tracing IDs, and custom headers to be propagated
//! to the upstream gRPC service.
//!
//! ## Scope
//! This module defines:
//! -   `forward_metadata`: Propagates HTTP headers to `tonic::metadata::MetadataMap`.
//! -   `grpc_timeout`: Parses `grpc-timeout` headers into `Duration`.
//!
//! ## Position in the Architecture
//! Called by generated code before making the gRPC request. It populates the `tonic::Request`
//! metadata from the incoming `http::Request` headers.
//!
//! ## Design Constraints
//! -   **Filtering**: Certain headers (e.g., `Content-Type`, `Host`) are filtered out to prevent
//!     interference with the gRPC transport.

use core::str::FromStr;
use core::time::Duration;
use http::Request;
use tonic::metadata::{MetadataKey, MetadataMap, MetadataValue};

/// Propagates HTTP headers from the incoming request to the gRPC metadata map.
///
/// This function iterates over the HTTP headers and converts them into gRPC metadata entries.
/// It automatically filters out transport-specific headers that should not be forwarded
/// (e.g., `Content-Type`, `Content-Length`, `Host`, `Connection`).
///
/// # Parameters
/// *   `req`: The incoming HTTP request.
/// *   `metadata`: The mutable gRPC metadata map to populate.
pub fn forward_metadata<B>(req: &Request<B>, metadata: &mut MetadataMap) {
    for (key, value) in req.headers() {
        let key_str = key.as_str();

        // Filter restricted headers that should not be propagated as metadata
        if key_str.eq_ignore_ascii_case("content-type")
            || key_str.eq_ignore_ascii_case("content-length")
            || key_str.eq_ignore_ascii_case("host")
            || key_str.eq_ignore_ascii_case("connection")
        {
            continue;
        }

        // Note: HTTP/2 field names are lowercased. Tonic expects lowercase keys.
        if let Ok(key_parsed) = MetadataKey::from_str(key_str) {
            // We use try_from on bytes to support binary metadata if needed,
            // though here we are just taking header bytes.
            if let Ok(val) = MetadataValue::try_from(value.as_bytes()) {
                metadata.insert(key_parsed, val);
            }
        }
    }
}

/// Parses the `grpc-timeout` header value into a `Duration`.
///
/// The format is a positive integer followed by a unit suffix:
/// -   `H`: Hours
/// -   `M`: Minutes
/// -   `S`: Seconds
/// -   `m`: Milliseconds
/// -   `u`: Microseconds
/// -   `n`: Nanoseconds
///
/// # Parameters
/// *   `val`: The header value string.
///
/// # Returns
/// An `Option<Duration>` if parsing is successful, otherwise `None`.
pub fn grpc_timeout(val: &str) -> Option<Duration> {
    if val.is_empty() {
        return None;
    }
    let (num, unit) = val.split_at(val.len() - 1);
    let n: u64 = num.parse().ok()?;
    match unit {
        "H" => Some(Duration::from_secs(n * 3600)),
        "M" => Some(Duration::from_secs(n * 60)),
        "S" => Some(Duration::from_secs(n)),
        "m" => Some(Duration::from_millis(n)),
        "u" => Some(Duration::from_micros(n)),
        "n" => Some(Duration::from_nanos(n)),
        _ => None,
    }
}
