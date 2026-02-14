//! # Metrics Layer (Legacy)
//!
//! This layer provides a simple mechanism for recording request metrics (duration, success/failure)
//! via a user-provided callback.
//!
//! ## Deprecation Warning
//! This module is deprecated in favor of industry-standard observability tools like `tower-http`'s
//! `TraceLayer` combined with the `tracing` crate or dedicated metrics libraries.
//! It is maintained for backward compatibility.

use crate::alloc::boxed::Box;
use crate::alloc::sync::Arc;
use crate::alloc::vec::Vec;
use crate::{GatewayError, GatewayRequest, GatewayResponse, GatewayResult};
use core::task::{Context, Poll};
use core::time::Duration;
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// A handler for recording metrics.
///
/// Functions of this type are invoked after a request has completed (or failed).
/// They receive the original request (context), the result (Response or Error), and the total duration.
///
/// **Note:** This is a legacy interface. New code should prefer `tower-http`'s `TraceLayer` or a dedicated metrics middleware.
pub type MetricsRecorder = Arc<dyn Fn(&GatewayRequest, &GatewayResult, Duration) + Send + Sync>;

/// A generic layer for recording metrics around the inner service execution.
///
/// This layer intercepts requests and responses, recording duration and result status using
/// the configured [MetricsRecorder].
#[derive(Clone)]
pub struct MetricsLayer<S> {
    pub(crate) inner: S,
    pub(crate) recorder: Option<MetricsRecorder>,
}

impl<S> MetricsLayer<S> {
    /// Creates a new `MetricsLayer`.
    pub fn new(inner: S, recorder: Option<MetricsRecorder>) -> Self {
        Self { inner, recorder }
    }
}

impl<S> Service<GatewayRequest> for MetricsLayer<S>
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
        let recorder = self.recorder.clone();

        // Capture request metadata for the recorder callback
        let method = req.method().clone();
        let uri = req.uri().clone();
        let headers = req.headers().clone();

        let start_time = std::time::Instant::now();
        let fut = self.inner.call(req);

        Box::pin(async move {
            let res = fut.await;
            let duration = start_time.elapsed();

            if let Some(rec) = recorder {
                // Reconstruct a partial request for context in the recorder
                let mut partial_req = http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Vec::new())
                    .unwrap();
                *partial_req.headers_mut() = headers;

                rec(&partial_req, &res, duration);
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
    async fn test_metrics_layer_records() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let recorder: MetricsRecorder = Arc::new(move |_, res, dur| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            assert!(res.is_ok());
            assert!(dur.as_nanos() > 0);
        });

        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = MetricsLayer::new(service, Some(recorder));
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_metrics_layer_no_recorder() {
        let service = tower::service_fn(|_req: GatewayRequest| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = MetricsLayer::new(service, None);
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
        // Should not crash
    }
}
