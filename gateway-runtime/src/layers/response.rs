//! # Response Modification Layer
//!
//! This layer executes registered [ResponseModifier] hooks after the request has been
//! processed by the inner service.
//!
//! This is typically used to inject headers (e.g., standard security headers),
//! rewrite status codes (e.g., mapping gRPC metadata to HTTP status), or perform
//! post-processing logging.

use crate::alloc::boxed::Box;
use crate::alloc::sync::Arc;
use crate::alloc::vec::Vec;
use crate::{GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// A handler for modifying HTTP responses before they are sent.
///
/// Functions of this type are invoked after the inner service returns a successful response.
/// They receive a read-only view of the original request (headers/metadata) and a mutable
/// reference to the response, allowing for header injection or status modification.
pub type ResponseModifier = Arc<dyn Fn(&GatewayRequest, &mut GatewayResponse) + Send + Sync>;

/// A Tower middleware that applies response modifiers.
#[derive(Clone)]
pub struct ResponseLayer<S> {
    inner: S,
    modifiers: Vec<ResponseModifier>,
}

impl<S> ResponseLayer<S> {
    /// Creates a new `ResponseLayer`.
    ///
    /// # Parameters
    /// *   `inner`: The inner service.
    /// *   `modifiers`: A list of functions that can mutate the response.
    pub fn new(inner: S, modifiers: Vec<ResponseModifier>) -> Self {
        Self { inner, modifiers }
    }
}

impl<S> Service<GatewayRequest> for ResponseLayer<S>
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

    fn call(&mut self, req: GatewayRequest) -> Self::Future {
        // Capture request context (Method, URI, Headers) for use by the modifiers.
        // The body is not available as it is consumed by the inner service.
        let method = req.method().clone();
        let uri = req.uri().clone();
        let headers = req.headers().clone();

        let modifiers = self.modifiers.clone();
        let fut = self.inner.call(req);

        Box::pin(async move {
            let mut resp = fut.await?;

            // Execute all modifiers on the successful response
            if !modifiers.is_empty() {
                let mut partial_req = http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Vec::new())
                    .unwrap();
                *partial_req.headers_mut() = headers;

                for modifier in &modifiers {
                    modifier(&partial_req, &mut resp);
                }
            }

            Ok(resp)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GatewayError;
    use http::StatusCode;
    use http_body_util::BodyExt;
    use http_body_util::Full;

    #[tokio::test]
    async fn test_response_layer_modifies() {
        let modifier: ResponseModifier = Arc::new(|_, resp| {
            resp.headers_mut().insert("x-modified", "true".parse().unwrap());
        });

        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(
                BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))
            ))
        });

        let mut layer = ResponseLayer::new(service, vec![modifier]);
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        let resp = layer.call(req).await.unwrap();
        assert_eq!(resp.headers().get("x-modified").unwrap(), "true");
    }

    #[tokio::test]
    async fn test_response_layer_multiple_modifiers() {
        let m1: ResponseModifier = Arc::new(|_, resp| {
            resp.headers_mut().insert("h1", "v1".parse().unwrap());
        });
        let m2: ResponseModifier = Arc::new(|_, resp| {
            resp.headers_mut().insert("h2", "v2".parse().unwrap());
        });

        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = ResponseLayer::new(service, vec![m1, m2]);
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        let resp = layer.call(req).await.unwrap();
        assert_eq!(resp.headers().get("h1").unwrap(), "v1");
        assert_eq!(resp.headers().get("h2").unwrap(), "v2");
    }

    #[tokio::test]
    async fn test_response_layer_access_request_context() {
        let modifier: ResponseModifier = Arc::new(|req, resp| {
            if req.headers().contains_key("x-trigger") {
                *resp.status_mut() = StatusCode::ACCEPTED;
            }
        });

        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = ResponseLayer::new(service, vec![modifier]);

        // Request with trigger
        let req = http::Request::builder().header("x-trigger", "1").body(crate::alloc::vec::Vec::new()).unwrap();
        let resp = layer.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        // Request without trigger
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();
        let resp = layer.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
