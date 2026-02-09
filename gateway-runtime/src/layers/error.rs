//! # Error Handling Layer
//!
//! This layer intercepts errors returned by the inner service and converts them into
//! valid HTTP responses using a configured [ErrorHandler].
//!
//! This is crucial for returning user-friendly error messages (e.g., JSON) instead of
//! raw server errors or dropped connections.

use crate::alloc::boxed::Box;
use crate::gateway::ErrorHandler;
use crate::{GatewayError, GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

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
