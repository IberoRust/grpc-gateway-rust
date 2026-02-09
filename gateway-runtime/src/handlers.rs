//! # Custom Handlers
//!
//! This module provides specific implementations of handlers mirroring the `grpc-gateway` (Go) ecosystem.
//! These handlers offer extended functionality beyond the `defaults` module, such as
//! advanced error formatting and context extraction.
//!
//! ## Components
//! -   `custom_http_error`: Returns detailed JSON error responses matching Go's default error format.
//! -   `http_response_modifier`: Modifies the HTTP response status code based on gRPC metadata (`x-http-code`).
//! -   Header matchers: specialized filtering for Auth/Refresh tokens.

use crate::alloc::string::{String, ToString};
use crate::defaults::default_error_handler;
use crate::errors::GatewayError;
use crate::{GatewayRequest, GatewayResponse};
use http::StatusCode;
use http_body_util::BodyExt;

/// A custom HTTP error handler that returns JSON error responses.
///
/// This handler maps gRPC status codes to HTTP status codes and returns a JSON body with:
/// - `message`: The error message.
/// - `status_code`: The HTTP status code.
/// - `title`: A title for the error (default "Error").
///
/// It falls back to `default_error_handler` for 2xx codes or codes outside the valid HTTP range [200, 505].
pub fn custom_http_error(req: &GatewayRequest, err: GatewayError) -> GatewayResponse {
    let status = match &err {
        GatewayError::Upstream(s) => crate::errors::map_code_to_status(s.code()),
        GatewayError::Http(_) => StatusCode::INTERNAL_SERVER_ERROR,
        GatewayError::Custom(s, _) => *s,
        GatewayError::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        GatewayError::NotFound => StatusCode::NOT_FOUND,
        GatewayError::Encoding(_) => StatusCode::BAD_REQUEST,
    };

    let code = status.as_u16();

    // Delegate to default handler for success codes or invalid HTTP ranges
    if code <= 300 || code > 505 {
        return default_error_handler(req, err);
    }

    // JSON Error Response Body
    #[derive(serde::Serialize)]
    struct ErrorMessage {
        message: String,
        status_code: u16,
        title: String,
    }

    let msg = ErrorMessage {
        message: err.to_string(),
        status_code: code,
        title: "Error".to_string(),
    };

    let body_bytes = serde_json::to_vec(&msg).unwrap_or_default();
    let body = http_body_util::BodyExt::boxed_unsync(
        http_body_util::Full::new(crate::bytes::Bytes::from(body_bytes))
            .map_err(|_| unreachable!()),
    );

    http::Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(body)
        .unwrap()
}

/// Modifies the HTTP response based on metadata headers.
///
/// Specifically, it looks for an `x-http-code` header (which may have been mapped from
/// gRPC metadata by the upstream service) and uses it to override the HTTP status code.
/// It then removes the header to prevent leaking implementation details.
pub fn http_response_modifier(_req: &GatewayRequest, resp: &mut GatewayResponse) {
    if let Some(val) = resp.headers().get("x-http-code") {
        if let Ok(s) = val.to_str() {
            if let Ok(code) = s.parse::<u16>() {
                if let Ok(status) = StatusCode::from_u16(code) {
                    *resp.status_mut() = status;
                }
            }
        }
        // Cleanup headers
        resp.headers_mut().remove("x-http-code");
        resp.headers_mut()
            .remove("grpc-metadata-x-http-status-code");
    }
}

/// Incoming header matcher that filters Auth headers.
///
/// - `Authorization`: Dropped (returns `None`).
/// - `Refresh`: Renamed to `x-refresh-token`.
/// - Other: Forwarded as lowercased.
pub fn incoming_header_matcher(key: &str) -> Option<String> {
    let key_lower = key.to_lowercase();
    match key_lower.as_str() {
        "authorization" => None,
        "refresh" => Some("x-refresh-token".to_string()),
        _ => Some(key_lower),
    }
}

/// Outgoing header matcher (Pass-through).
///
/// Forwards all headers in lowercase.
pub fn outgoing_header_matcher(key: &str) -> Option<String> {
    Some(key.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::vec::Vec;

    #[test]
    fn test_custom_http_error_json() {
        let req = http::Request::builder().body(Vec::new()).unwrap();
        let err = GatewayError::NotFound;
        let resp = custom_http_error(&req, err);

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_custom_http_error_fallback() {
        let req = http::Request::builder().body(Vec::new()).unwrap();
        let err = GatewayError::Custom(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal error".to_string(),
        );
        let resp = custom_http_error(&req, err);
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_http_response_modifier() {
        let req = http::Request::builder().body(Vec::new()).unwrap();
        let mut resp = http::Response::builder()
            .status(StatusCode::OK)
            .header("x-http-code", "400")
            .body(BodyExt::boxed_unsync(
                http_body_util::Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()),
            ))
            .unwrap();

        http_response_modifier(&req, &mut resp);
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(resp.headers().get("x-http-code").is_none());
    }

    #[test]
    fn test_incoming_header_matcher() {
        assert_eq!(incoming_header_matcher("Authorization"), None);
        assert_eq!(
            incoming_header_matcher("Refresh"),
            Some("x-refresh-token".to_string())
        );
        assert_eq!(incoming_header_matcher("Other"), Some("other".to_string()));
    }
}
