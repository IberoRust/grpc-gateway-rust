//! # Response Modification Layer
//!
//! This layer executes registered [ResponseModifier] hooks after the request has been
//! processed by the inner service.
//!
//! This is typically used to inject headers (e.g., standard security headers),
//! rewrite status codes (e.g., mapping gRPC metadata to HTTP status), or perform
//! post-processing logging.

use crate::alloc::boxed::Box;
use crate::alloc::vec::Vec;
use crate::gateway::ResponseModifier;
use crate::{GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

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
