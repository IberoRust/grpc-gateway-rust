//! # Governance Layer
//!
//! This layer provides advanced governance and protection mechanisms for the gateway.
//!
//! ## Features
//! *   **Global Concurrency Limits**: Restricts the total number of simultaneous connections/requests.
//! *   **Request Timeouts**: Enforces a strict time limit on request processing.
//! *   **Request Body Limits**: Rejects requests with payloads exceeding a configured size.
//! *   **Response Body Limits**: Terminates responses that exceed a configured size (preventing large downloads).
//! *   **Rate Limiting**: Token-bucket based request rate limiting (Requests Per Second).
//! *   **Load Shedding**: Automatically rejects requests when the service is overloaded (not ready).
//! *   **Retries**: Configurable retry policy for transient failures (e.g., 503, upstream errors).

use crate::alloc::boxed::Box;
use crate::{GatewayError, GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use core::time::Duration;
use http::StatusCode;
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// Configuration for the Governance Layer.
#[derive(Debug, Clone, Default)]
pub struct GovernanceConfig {
    /// Maximum number of concurrent requests allowed.
    pub connection_limit: Option<usize>,
    /// Maximum duration allowed for a request to complete.
    pub request_timeout: Option<Duration>,
    /// Maximum size (in bytes) allowed for the request body.
    pub max_request_body_size: Option<usize>,
    /// Maximum size (in bytes) allowed for the response body.
    pub max_response_body_size: Option<usize>,
    /// Rate limit in requests per second.
    pub rate_limit_per_second: Option<u64>,
    /// Enable automatic load shedding (fail fast when overloaded).
    pub enable_load_shedding: bool,
    /// Number of retries for transient failures.
    pub retry_count: Option<usize>,
}

/// A retry policy for the Gateway.
///
/// Retries the request if:
/// 1. The error is an Upstream error (gRPC status).
/// 2. The HTTP status is 503 (Service Unavailable) or 502 (Bad Gateway).
#[derive(Clone)]
pub struct GatewayRetryPolicy {
    remaining_attempts: usize,
}

impl GatewayRetryPolicy {
    pub fn new(attempts: usize) -> Self {
        Self {
            remaining_attempts: attempts,
        }
    }
}

impl tower::retry::Policy<GatewayRequest, GatewayResponse, GatewayError> for GatewayRetryPolicy {
    type Future = futures::future::Ready<()>;

    fn retry(
        &mut self,
        _req: &mut GatewayRequest,
        result: &mut Result<GatewayResponse, GatewayError>,
    ) -> Option<Self::Future> {
        if self.remaining_attempts == 0 {
            return None;
        }

        match result {
            Ok(resp) => {
                // Retry on server errors
                if resp.status() == StatusCode::SERVICE_UNAVAILABLE
                    || resp.status() == StatusCode::BAD_GATEWAY
                {
                    self.remaining_attempts -= 1;
                    Some(futures::future::ready(()))
                } else {
                    None
                }
            }
            Err(GatewayError::Upstream(_)) => {
                self.remaining_attempts -= 1;
                Some(futures::future::ready(()))
            },
            Err(_) => None,
        }
    }

    fn clone_request(&mut self, req: &GatewayRequest) -> Option<GatewayRequest> {
        // GatewayRequest is http::Request<Vec<u8>>. Vec<u8> is cloneable.
        // Cloning the body is expensive but necessary for retries.
        // Since we are buffering the body in VecBodyToVecService anyway, this is the cost of retries.

        let mut new_req = http::Request::builder()
            .method(req.method().clone())
            .uri(req.uri().clone())
            .version(req.version());

        for (k, v) in req.headers() {
            new_req.headers_mut().unwrap().insert(k, v.clone());
        }

        new_req.body(req.body().clone()).ok()
    }
}

/// A layer that enforces body size limits on requests and responses.
///
/// Note: Timeout and Concurrency limits are applied using standard `tower` layers
/// constructed in `Gateway::into_service`, but this layer handles the logic specifically
/// for `Vec<u8>` request bodies and `BoxBody` response streams.
#[derive(Clone)]
pub struct BodyLimitLayer<S> {
    inner: S,
    max_req: Option<usize>,
    max_resp: Option<usize>,
}

impl<S> BodyLimitLayer<S> {
    pub fn new(inner: S, max_req: Option<usize>, max_resp: Option<usize>) -> Self {
        Self {
            inner,
            max_req,
            max_resp,
        }
    }
}

impl<S> Service<GatewayRequest> for BodyLimitLayer<S>
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
        // Enforce Request Body Limit
        if let Some(limit) = self.max_req {
            if req.body().len() > limit {
                return Box::pin(async move {
                    Err(GatewayError::Custom(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "Request body too large".into(),
                    ))
                });
            }
        }

        let max_resp = self.max_resp;
        let fut = self.inner.call(req);

        Box::pin(async move {
            let resp = fut.await?;

            // Enforce Response Body Limit
            if let Some(limit) = max_resp {
                // We map the body to a LimitedBody wrapper.
                // Since GatewayResponse uses UnsyncBoxBody, we need to wrap and re-box.
                let (parts, body) = resp.into_parts();
                let limited_body = LimitedBody {
                    inner: body,
                    remaining: limit,
                };

                // Re-box safely
                let boxed_body = http_body_util::combinators::UnsyncBoxBody::new(Box::new(limited_body));
                Ok(http::Response::from_parts(parts, boxed_body))
            } else {
                Ok(resp)
            }
        })
    }
}

/// A wrapper body that enforces a maximum byte limit.
struct LimitedBody<B> {
    inner: B,
    remaining: usize,
}

impl<B> http_body::Body for LimitedBody<B>
where
    B: http_body::Body<Data = crate::bytes::Bytes, Error = GatewayError> + Unpin,
{
    type Data = crate::bytes::Bytes;
    type Error = GatewayError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let res = Pin::new(&mut self.inner).poll_frame(cx);
        match res {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    if data.len() > self.remaining {
                        return Poll::Ready(Some(Err(GatewayError::Custom(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            "Response body limit exceeded".into(),
                        ))));
                    }
                    self.remaining -= data.len();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Full};
    use tower::retry::Policy;

    #[tokio::test]
    async fn test_req_body_limit_exceeded() {
        let service = tower::service_fn(|_| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(
                BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))
            ))
        });

        let mut layer = BodyLimitLayer::new(service, Some(5), None);
        let req = http::Request::builder().body(vec![0u8; 10]).unwrap(); // 10 bytes > 5

        let err = layer.call(req).await.unwrap_err();
        match err {
            GatewayError::Custom(status, msg) => {
                assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
                assert_eq!(msg, "Request body too large");
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[tokio::test]
    async fn test_req_body_limit_ok() {
        let service = tower::service_fn(|_| async {
            Ok::<GatewayResponse, GatewayError>(http::Response::new(
                BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))
            ))
        });

        let mut layer = BodyLimitLayer::new(service, Some(15), None);
        let req = http::Request::builder().body(vec![0u8; 10]).unwrap();

        assert!(layer.call(req).await.is_ok());
    }

    #[test]
    fn test_retry_policy() {
        // Test case 1: Retry on Service Unavailable
        {
            let mut policy = GatewayRetryPolicy::new(1);
            let mut req = http::Request::builder().body(Vec::new()).unwrap();
            let mut resp_res: Result<GatewayResponse, GatewayError> = Ok(http::Response::builder().status(StatusCode::SERVICE_UNAVAILABLE).body(
                 BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))
            ).unwrap());

            assert!(policy.retry(&mut req, &mut resp_res).is_some());
        }

        // Test case 2: Should NOT retry on OK
        {
            let mut policy = GatewayRetryPolicy::new(1);
            let mut req = http::Request::builder().body(Vec::new()).unwrap();
            let mut resp_ok_res: Result<GatewayResponse, GatewayError> = Ok(http::Response::builder().status(StatusCode::OK).body(
                 BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))
            ).unwrap());
            assert!(policy.retry(&mut req, &mut resp_ok_res).is_none());
        }

        // Test case 3: Should retry on Upstream error
        {
            let mut policy = GatewayRetryPolicy::new(1);
            let mut req = http::Request::builder().body(Vec::new()).unwrap();
            let mut err_res = Err(GatewayError::Upstream(tonic::Status::unavailable("fail")));
            assert!(policy.retry(&mut req, &mut err_res).is_some());
        }
    }
}
