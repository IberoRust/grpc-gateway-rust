//! # Error Handling Layer
//!
//! This layer intercepts errors returned by the inner service and converts them into
//! valid HTTP responses using a configured [ErrorHandler].
//!
//! This is crucial for returning user-friendly error messages (e.g., JSON) instead of
//! raw server errors or dropped connections.

use crate::alloc::boxed::Box;
use crate::alloc::sync::Arc;
use crate::{GatewayError, GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// A handler for converting errors into HTTP responses.
///
/// Implementations of this function type are responsible for mapping domain-specific
/// [GatewayError]s into user-facing [GatewayResponse]s (e.g., setting status codes, JSON bodies).
pub type ErrorHandler = Arc<dyn Fn(&GatewayRequest, GatewayError) -> GatewayResponse + Send + Sync>;

/// A Tower middleware that handles errors from the inner service.
#[derive(Clone)]
pub struct ErrorLayer<S> {
    inner: S,
    handler: Option<ErrorHandler>,
}

impl<S> ErrorLayer<S> {
    /// Creates a new `ErrorLayer`.
    ///
    /// # Parameters
    /// *   `inner`: The inner service to wrap.
    /// *   `handler`: An optional custom error handler. If `None`, errors are propagated.
    pub fn new(inner: S, handler: Option<ErrorHandler>) -> Self {
        Self { inner, handler }
    }
}

impl<S> Service<GatewayRequest> for ErrorLayer<S>
where
    S: Service<GatewayRequest, Response = GatewayResponse, Error = GatewayError>,
    S::Future: Send + 'static,
{
    type Response = GatewayResponse;
    type Error = GatewayError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: GatewayRequest) -> Self::Future {
        // Capture minimal request context (Method, URI, Headers) to pass to the error handler.
        // The body is not cloned to avoid performance penalties.
        let method = req.method().clone();
        let uri = req.uri().clone();
        let headers = req.headers().clone();

        let handler_clone = self.handler.clone();

        let fut = self.inner.call(req);

        Box::pin(async move {
            match fut.await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    if let Some(h) = handler_clone {
                        // Reconstruct a partial request for the handler context.
                        // The body is empty since the original request has been consumed.
                        let mut partial_req = http::Request::builder()
                            .method(method)
                            .uri(uri)
                            .body(crate::alloc::vec::Vec::new())
                            .unwrap();
                        *partial_req.headers_mut() = headers;

                        // Execute the custom error handler
                        Ok(h(&partial_req, err))
                    } else {
                        // Propagate the error if no handler is configured
                        Err(err)
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{StatusCode, Response};
    use http_body_util::{Full, BodyExt};
    use crate::alloc::string::ToString;

    #[tokio::test]
    async fn test_error_layer_catches_error() {
        // Mock service that always fails
        let service = tower::service_fn(|_req: GatewayRequest| async {
            Err(GatewayError::NotFound)
        });

        // Handler that converts error to 404 response
        let handler: ErrorHandler = Arc::new(|_, err| {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::from(err.to_string())).map_err(|_| unreachable!())))
                .unwrap()
        });

        let mut layer = ErrorLayer::new(service, Some(handler));
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        let resp = layer.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_error_layer_propagates_ok() {
        // Mock service that succeeds
        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok(Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::from("ok")).map_err(|_| unreachable!()))))
        });

        let handler: ErrorHandler = Arc::new(|_, _| {
            panic!("Handler should not be called");
        });

        let mut layer = ErrorLayer::new(service, Some(handler));
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        let resp = layer.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_error_layer_propagates_error_without_handler() {
        let service = tower::service_fn(|_req: GatewayRequest| async {
            Err(GatewayError::MethodNotAllowed)
        });

        let mut layer = ErrorLayer::new(service, None);
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        let res = layer.call(req).await;
        assert!(res.is_err());
    }
}
