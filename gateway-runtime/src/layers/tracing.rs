//! # Tracing Layer (Internal)
//!
//! This layer provides a simple mechanism for tracing request lifecycles via start and end hooks.
//! It allows passing opaque context from the start of a request to its completion.
//!
//! ## Deprecation Warning
//! This module is deprecated in favor of `tower-http`'s `TraceLayer`.
//! It is maintained for backward compatibility.

use crate::alloc::boxed::Box;
use crate::alloc::sync::Arc;
use crate::{GatewayError, GatewayRequest, GatewayResponse, GatewayResult};
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// A handler for tracing start.
///
/// Invoked before the request is processed. It returns an opaque token (boxed `Any`)
/// which is passed to the [TracingEndHandler].
pub type TracingStartHandler =
    Arc<dyn Fn(&GatewayRequest) -> Box<dyn core::any::Any + Send> + Send + Sync>;

/// A handler for tracing end.
///
/// Invoked after the request completes. It receives the opaque token from the start handler
/// and the result of the operation.
pub type TracingEndHandler =
    Arc<dyn Fn(Box<dyn core::any::Any + Send>, &GatewayResult) + Send + Sync>;

/// A generic layer for wrapping execution with tracing start/end hooks.
///
/// This layer allows custom logic to be executed before and after a request is processed,
/// maintaining context via an opaque token.
///
/// **Note:** This is a deprecated interface. New code should prefer `tower-http`'s `TraceLayer`.
#[derive(Clone)]
pub struct TraceLayer<S> {
    pub(crate) inner: S,
    pub(crate) start: Option<TracingStartHandler>,
    pub(crate) end: Option<TracingEndHandler>,
}

impl<S> TraceLayer<S> {
    /// Creates a new `TraceLayer`.
    pub fn new(inner: S, start: Option<TracingStartHandler>, end: Option<TracingEndHandler>) -> Self {
        Self { inner, start, end }
    }
}

impl<S> Service<GatewayRequest> for TraceLayer<S>
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
        let token = self.start.as_ref().map(|start| start(&req));

        let end = self.end.clone();
        let fut = self.inner.call(req);

        Box::pin(async move {
            let res = fut.await;
            if let Some(end_handler) = end {
                if let Some(t) = token {
                    end_handler(t, &res);
                }
            }
            res
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    use http_body_util::Full;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_trace_layer_callbacks() {
        let start_count = Arc::new(AtomicUsize::new(0));
        let end_count = Arc::new(AtomicUsize::new(0));
        let s_c = start_count.clone();
        let e_c = end_count.clone();

        let start: TracingStartHandler = Arc::new(move |_| {
            s_c.fetch_add(1, Ordering::SeqCst);
            Box::new(100u32) // Pass context
        });

        let end: TracingEndHandler = Arc::new(move |ctx, _| {
            let val = *ctx.downcast::<u32>().unwrap();
            assert_eq!(val, 100);
            e_c.fetch_add(1, Ordering::SeqCst);
        });

        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = TraceLayer::new(service, Some(start), Some(end));
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
        assert_eq!(start_count.load(Ordering::SeqCst), 1);
        assert_eq!(end_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_trace_layer_no_handlers() {
        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = TraceLayer::new(service, None, None);
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
    }
}
