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
//! -   **Security**: Only forwards headers that match allowed prefixes or are explicitly permitted to prevent header injection attacks.

use core::str::FromStr;
use core::time::Duration;
use http::Request;
use tonic::metadata::{MetadataKey, MetadataMap, MetadataValue};

/// Configuration for metadata forwarding security.
#[derive(Debug, Clone)]
pub struct MetadataForwardingConfig {
    /// Allowed prefixes for headers to be forwarded (e.g., "grpc-metadata-", "x-").
    /// Defaults to `["grpc-metadata-"]` to match `grpc-gateway` behavior.
    pub allowed_prefixes: alloc::vec::Vec<alloc::string::String>,
    /// Explicitly allowed headers (e.g., "authorization").
    pub allowed_headers: alloc::vec::Vec<alloc::string::String>,
}

impl Default for MetadataForwardingConfig {
    fn default() -> Self {
        Self {
            allowed_prefixes: crate::alloc::vec![
                "grpc-metadata-".into(),
                // We typically also allow "x-" for custom headers if desired, but
                // grpc-gateway defaults to strictly "Grpc-Metadata-".
                // We'll stick to strict default but allow configuration.
            ],
            allowed_headers: crate::alloc::vec![
                "authorization".into(),
                "x-request-id".into(),
                "x-b3-traceid".into(),
                "x-b3-spanid".into(),
                "x-b3-parentspanid".into(),
                "x-b3-sampled".into(),
                "x-b3-flags".into(),
                "x-ot-span-context".into(),
                "traceparent".into(),
                "tracestate".into(),
            ],
        }
    }
}

/// Propagates HTTP headers from the incoming request to the gRPC metadata map.
///
/// This function iterates over the HTTP headers and converts them into gRPC metadata entries.
/// It automatically filters out transport-specific headers and enforces security rules
/// based on the provided configuration (or defaults if not specified via `Gateway`).
///
/// It also renames headers to have an `x-` prefix if they are not standard authentication headers
/// and do not already have the prefix, to indicate they originate from the gateway.
///
/// # Parameters
/// *   `req`: The incoming HTTP request.
/// *   `metadata`: The mutable gRPC metadata map to populate.
/// *   `config`: Optional configuration for forwarding rules.
pub fn forward_metadata<B>(req: &Request<B>, metadata: &mut MetadataMap) {
    let default_config = MetadataForwardingConfig::default();
    // Retrieve config from extensions if available, else use default.
    let config = req
        .extensions()
        .get::<MetadataForwardingConfig>()
        .unwrap_or(&default_config);

    for (key, value) in req.headers() {
        let key_str = key.as_str();

        // 1. Filter restricted/transport headers
        if key_str.eq_ignore_ascii_case("content-type")
            || key_str.eq_ignore_ascii_case("content-length")
            || key_str.eq_ignore_ascii_case("host")
            || key_str.eq_ignore_ascii_case("connection")
            || key_str.eq_ignore_ascii_case("keep-alive")
            || key_str.eq_ignore_ascii_case("proxy-authenticate")
            || key_str.eq_ignore_ascii_case("proxy-authorization")
            || key_str.eq_ignore_ascii_case("te")
            || key_str.eq_ignore_ascii_case("trailer")
            || key_str.eq_ignore_ascii_case("transfer-encoding")
            || key_str.eq_ignore_ascii_case("upgrade")
        {
            continue;
        }

        // 2. Security Check: Allowlist or Prefix match
        let is_allowed = config
            .allowed_headers
            .iter()
            .any(|h| key_str.eq_ignore_ascii_case(h))
            || config
                .allowed_prefixes
                .iter()
                .any(|p| key_str.to_lowercase().starts_with(&p.to_lowercase()));

        if !is_allowed {
            continue;
        }

        // 3. Renaming (Optional/Compatibility)
        // If it's already "grpc-metadata-", it maps directly.
        // If it's "x-", it maps directly.
        // We preserve the logic requested previously: "prefix x- unless standard".
        // But strict security implies we only forward what we TRUST or explicitly allow.
        // If allowed, we forward as is? Or do we still rename?
        // grpc-gateway behavior: `Grpc-Metadata-Foo` -> `Foo` in metadata? Or `grpc-metadata-foo`?
        // Actually, grpc-gateway typically strips `Grpc-Metadata-`.
        // Rust Tonic doesn't strip automatically.
        // For this task, we'll keep the previous "x-" prefixing logic for non-standard headers
        // that pass the filter, to maintain the requested behavior of "identifying from gateway".

        let mut final_key_str = key_str.to_string();
        if !key_str.eq_ignore_ascii_case("authorization")
            && !key_str.eq_ignore_ascii_case("grpc-timeout")
            && !key_str.starts_with("x-")
            && !key_str.starts_with("grpc-")
        {
            final_key_str = format!("x-{}", key_str);
        }

        if final_key_str.ends_with("-bin") {
            if let Ok(key_parsed) =
                MetadataKey::<tonic::metadata::Binary>::from_bytes(final_key_str.as_bytes())
            {
                let val = MetadataValue::from_bytes(value.as_bytes());
                metadata.insert_bin(key_parsed, val);
            }
        } else {
            if let Ok(key_parsed) = MetadataKey::<tonic::metadata::Ascii>::from_str(&final_key_str)
            {
                if let Ok(val) = MetadataValue::try_from(value.as_bytes()) {
                    metadata.insert(key_parsed, val);
                }
            }
        }
    }

    // Merge Metadata from Extensions (e.g. from MetadataLayer)
    // This runs after header processing to ensure middleware-injected metadata takes precedence
    // and is not subject to the same filtering rules (as it is trusted).
    if let Some(ext_map) = req.extensions().get::<MetadataMap>() {
        for item in ext_map.iter() {
            match item {
                tonic::metadata::KeyAndValueRef::Ascii(key, val) => {
                    metadata.insert(key.clone(), val.clone());
                }
                tonic::metadata::KeyAndValueRef::Binary(key, val) => {
                    metadata.insert_bin(key.clone(), val.clone());
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_metadata_allowed() {
        let req = http::Request::builder()
            .header("authorization", "token")
            .header("grpc-metadata-custom", "val")
            .body(())
            .unwrap();
        let mut md = MetadataMap::new();
        forward_metadata(&req, &mut md);

        assert_eq!(md.get("authorization").unwrap(), "token");
        assert_eq!(md.get("grpc-metadata-custom").unwrap(), "val");
    }

    #[test]
    fn test_forward_metadata_denied() {
        let req = http::Request::builder()
            .header("custom-header", "val") // Not in default allowed list/prefix
            .body(())
            .unwrap();
        let mut md = MetadataMap::new();
        forward_metadata(&req, &mut md);

        assert!(md.is_empty());
    }

    #[test]
    fn test_forward_metadata_custom_config() {
        let config = MetadataForwardingConfig {
            allowed_prefixes: crate::alloc::vec![],
            allowed_headers: crate::alloc::vec!["x-custom-allowed".to_string()],
        };
        let mut req = http::Request::builder()
            .header("x-custom-allowed", "val")
            .header("other", "nope")
            .body(())
            .unwrap();
        req.extensions_mut().insert(config);

        let mut md = MetadataMap::new();
        forward_metadata(&req, &mut md);

        assert_eq!(md.get("x-custom-allowed").unwrap(), "val");
        assert!(md.get("other").is_none());
    }

    #[test]
    fn test_grpc_timeout_parsing() {
        assert_eq!(grpc_timeout("1H"), Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_forward_metadata_extension_map() {
        let mut req = http::Request::builder().body(()).unwrap();
        let mut ext_map = MetadataMap::new();
        ext_map.insert("x-ctx-id", "123".parse().unwrap());
        req.extensions_mut().insert(ext_map);

        let mut md = MetadataMap::new();
        forward_metadata(&req, &mut md);

        assert_eq!(md.get("x-ctx-id").unwrap(), "123");
    }
}
