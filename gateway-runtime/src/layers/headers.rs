//! # Header Processing Layer
//!
//! This layer intercepts incoming requests and outgoing responses to filter or transform
//! HTTP headers using configured [HeaderMatcher](crate::gateway::HeaderMatcher) functions.
//!
//! This allows for renaming headers (e.g., `Authorization` -> `x-auth-token`) or stripping
//! sensitive/unwanted headers before they reach the application logic or the client.

use crate::alloc::boxed::Box;
use crate::gateway::HeaderMatcher;
use crate::{GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

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
