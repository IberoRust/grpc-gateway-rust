//! # Request Body Adapters
//!
//! This module provides adapter types to bridge the gap between `gateway-runtime`'s internal
//! request representation and the standard `http_body::Body` trait used by the `tower` ecosystem.
//!
//! ## Problem
//! The generated code and core `RouterService` expect `GatewayRequest`, which is defined as
//! `http::Request<Vec<u8>>`. However, advanced middleware (like `tower-http`) expects
//! `http::Request<B>` where `B: http_body::Body`. `Vec<u8>` does not implement `Body` directly
//! in a way that satisfies streaming requirements or advanced buffering logic.
//!
//! ## Solution
//! *   **[VecBody]**: A wrapper struct that implements `http_body::Body` for `Vec<u8>`. It yields
//!     the entire vector as a single data frame.
//! *   **[VecBodyToVecService]**: A middleware service that accepts `http::Request<VecBody>`,
//!     extracts the underlying `Vec<u8>`, and forwards a `GatewayRequest` to the inner service.
//!
//! ## Usage
//! These components are primarily used within `Gateway::into_service()` to wrap the middleware stack,
//! allowing standard Tower layers to operate on the request before it reaches the core routing logic.

use crate::alloc::string::ToString;
use crate::alloc::vec::Vec;
use crate::{GatewayError, GatewayRequest, GatewayResponse};
use core::task::{Context, Poll};
use std::pin::Pin;
use tower::Service;

/// A simple wrapper around Vec<u8> that implements http_body::Body.
///
/// This wrapper allows `gateway-runtime`'s `Vec<u8>`-based requests to be processed
/// by `tower-http` middleware, which expects types implementing the `Body` trait.
pub struct VecBody(pub(crate) Option<Vec<u8>>);

impl http_body::Body for VecBody {
    type Data = crate::bytes::Bytes;
    type Error = GatewayError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        if let Some(data) = self.0.take() {
            Poll::Ready(Some(Ok(http_body::Frame::data(crate::bytes::Bytes::from(
                data,
            )))))
        } else {
            Poll::Ready(None)
        }
    }
}

impl http_body::Body for &VecBody {
    type Data = crate::bytes::Bytes;
    type Error = GatewayError;

    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(None)
    }
}

/// Adapter service to unwrap VecBody back to Vec<u8> for RouterService.
///
/// This service acts as a bridge, converting the `http::Request<VecBody>` (from `tower-http`)
/// back into the `GatewayRequest` (`Request<Vec<u8>>`) expected by the core router logic.
#[derive(Clone)]
pub struct VecBodyToVecService<S> {
    pub(crate) inner: S,
}

impl<S> VecBodyToVecService<S> {
    /// Creates a new adapter service.
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<http::Request<VecBody>> for VecBodyToVecService<S>
where
    S: Service<GatewayRequest, Response = GatewayResponse, Error = GatewayError>,
{
    type Response = GatewayResponse;
    type Error = GatewayError;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<VecBody>) -> Self::Future {
        let (parts, body) = req.into_parts();
        let vec = body.0.unwrap_or_default();
        let new_req = http::Request::from_parts(parts, vec);
        self.inner.call(new_req)
    }
}

/// Helper to wrap any `http_body::Body` into a standard `GatewayResponse` (`http::Response<BoxBody>`).
///
/// This utility ensures that responses from conditional middleware (like `CompressionLayer`)
/// are boxed into the uniform return type expected by `Gateway::into_service()`. It also maps
/// any body errors to `GatewayError::Custom`.
pub fn box_response_body<B>(res: http::Response<B>) -> GatewayResponse
where
    B: http_body::Body<Data = crate::bytes::Bytes> + 'static + Send,
    B::Error: core::fmt::Display + Send + Sync,
{
    use http_body_util::combinators::UnsyncBoxBody;
    use http_body_util::BodyExt;

    res.map(|b| {
        let b = b.map_err(|e| {
            GatewayError::Custom(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        });
        UnsyncBoxBody::new(b)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn test_vec_body_poll() {
        let data = b"test data".to_vec();
        let mut body = VecBody(Some(data.clone()));

        // First poll should return data
        let frame = body
            .frame()
            .await
            .expect("Should return frame")
            .expect("Should be OK");
        assert_eq!(frame.data_ref().unwrap().as_ref(), &data[..]);

        // Second poll should be None
        assert!(body.frame().await.is_none());
    }

    #[tokio::test]
    async fn test_vec_body_empty() {
        let mut body = VecBody(None);
        assert!(body.frame().await.is_none());
    }

    #[tokio::test]
    async fn test_vec_body_to_vec_service() {
        let mock_service = tower::service_fn(|req: GatewayRequest| async move {
            let body_len = req.body().len();
            Ok::<_, GatewayError>(
                http::Response::builder()
                    .header("x-len", body_len.to_string())
                    .body(http_body_util::BodyExt::boxed_unsync(
                        http_body_util::Full::new(crate::bytes::Bytes::new())
                            .map_err(|_| -> GatewayError { unreachable!() }),
                    ))
                    .unwrap(),
            )
        });

        let mut adapter = VecBodyToVecService::new(mock_service);
        let req = http::Request::builder()
            .body(VecBody(Some(b"hello".to_vec())))
            .unwrap();
        let resp = adapter.call(req).await.unwrap();

        assert_eq!(resp.headers().get("x-len").unwrap(), "5");
    }

    #[tokio::test]
    async fn test_box_response_body_wrapping() {
        let original_resp = http::Response::new(http_body_util::Full::new(
            crate::bytes::Bytes::from("test"),
        ));
        let boxed_resp = box_response_body(original_resp);

        // Verify structure (opaque check mainly since BoxBody is generic)
        assert_eq!(boxed_resp.status(), http::StatusCode::OK);
    }
}
