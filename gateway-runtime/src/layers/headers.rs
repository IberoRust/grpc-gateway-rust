//! # Header Processing Layer
//!
//! This layer intercepts incoming requests and outgoing responses to filter or transform
//! HTTP headers using configured [HeaderMatcher] functions.
//!
//! This allows for renaming headers (e.g., `Authorization` -> `x-auth-token`) or stripping
//! sensitive/unwanted headers before they reach the application logic or the client.

use crate::alloc::boxed::Box;
use crate::alloc::string::String;
use crate::alloc::sync::Arc;
use crate::{GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// A handler for matching and transforming headers.
///
/// It takes a header name (as a string slice) and returns an `Option<String>`.
/// *   `Some(new_name)`: Renames the header to `new_name` (or keeps it if identical).
/// *   `None`: Removes the header.
pub type HeaderMatcher = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A Tower middleware that applies header matching logic.
#[derive(Clone)]
pub struct HeaderLayer<S> {
    inner: S,
    incoming_matcher: Option<HeaderMatcher>,
    outgoing_matcher: Option<HeaderMatcher>,
}

impl<S> HeaderLayer<S> {
    /// Creates a new `HeaderLayer`.
    ///
    /// # Parameters
    /// *   `inner`: The inner service.
    /// *   `incoming`: Optional matcher for request headers.
    /// *   `outgoing`: Optional matcher for response headers.
    pub fn new(inner: S, incoming: Option<HeaderMatcher>, outgoing: Option<HeaderMatcher>) -> Self {
        Self {
            inner,
            incoming_matcher: incoming,
            outgoing_matcher: outgoing,
        }
    }
}

impl<S> Service<GatewayRequest> for HeaderLayer<S>
where
    S: Service<GatewayRequest, Response = GatewayResponse>,
    S::Future: Send + 'static,
{
    type Response = GatewayResponse;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: GatewayRequest) -> Self::Future {
        // Process Incoming Headers
        if let Some(matcher) = &self.incoming_matcher {
            let mut new_headers = http::HeaderMap::new();
            for (key, value) in req.headers() {
                if let Some(new_key) = matcher(key.as_str()) {
                    if let Ok(k) = http::header::HeaderName::from_bytes(new_key.as_bytes()) {
                        new_headers.insert(k, value.clone());
                    }
                }
            }
            *req.headers_mut() = new_headers;
        }

        let outgoing_matcher = self.outgoing_matcher.clone();
        let fut = self.inner.call(req);

        Box::pin(async move {
            let mut resp = fut.await?;

            // Process Outgoing Headers
            if let Some(matcher) = outgoing_matcher {
                let mut new_headers = http::HeaderMap::new();
                for (key, value) in resp.headers() {
                    if let Some(new_key) = matcher(key.as_str()) {
                        if let Ok(k) = http::header::HeaderName::from_bytes(new_key.as_bytes()) {
                            new_headers.insert(k, value.clone());
                        }
                    }
                }
                *resp.headers_mut() = new_headers;
            }

            Ok(resp)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GatewayError;
    use http_body_util::BodyExt;
    use http_body_util::Full;

    #[tokio::test]
    async fn test_header_layer_filters_incoming() {
        let matcher: HeaderMatcher = Arc::new(|key| {
            if key == "x-allowed" {
                Some(key.to_string())
            } else {
                None
            }
        });

        let service = tower::service_fn(|req: GatewayRequest| async move {
            assert!(req.headers().contains_key("x-allowed"));
            assert!(!req.headers().contains_key("x-forbidden"));
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = HeaderLayer::new(service, Some(matcher), None);
        let req = http::Request::builder()
            .header("x-allowed", "true")
            .header("x-forbidden", "true")
            .body(crate::alloc::vec::Vec::new())
            .unwrap();

        layer.call(req).await.unwrap();
    }

    #[tokio::test]
    async fn test_header_layer_renames_incoming() {
        let matcher: HeaderMatcher = Arc::new(|key| {
            if key == "old-name" {
                Some("new-name".to_string())
            } else {
                Some(key.to_string())
            }
        });

        let service = tower::service_fn(|req: GatewayRequest| async move {
            assert!(req.headers().contains_key("new-name"));
            assert!(!req.headers().contains_key("old-name"));
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = HeaderLayer::new(service, Some(matcher), None);
        let req = http::Request::builder()
            .header("old-name", "value")
            .body(crate::alloc::vec::Vec::new())
            .unwrap();

        layer.call(req).await.unwrap();
    }

    #[tokio::test]
    async fn test_header_layer_transforms_outgoing() {
        let matcher: HeaderMatcher = Arc::new(|key| {
            if key == "x-secret" {
                None
            } else if key == "x-internal" {
                Some("x-public".to_string())
            } else {
                Some(key.to_string())
            }
        });

        let service = tower::service_fn(|_req: GatewayRequest| async {
            let mut resp = http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!())));
            resp.headers_mut().insert("x-secret", "shhh".parse().unwrap());
            resp.headers_mut().insert("x-internal", "val".parse().unwrap());
            resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
            Ok::<GatewayResponse, GatewayError>(resp)
        });

        let mut layer = HeaderLayer::new(service, None, Some(matcher));
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        let resp = layer.call(req).await.unwrap();
        assert!(!resp.headers().contains_key("x-secret"));
        assert!(!resp.headers().contains_key("x-internal"));
        assert!(resp.headers().contains_key("x-public"));
        assert!(resp.headers().contains_key("content-type"));
    }
}
