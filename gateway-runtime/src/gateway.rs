//! # Gateway Builder and Service
//!
//! ## Purpose
//! This module provides the `Gateway` builder struct, which is the primary entry point for configuring
//! and constructing the runtime service stack. It orchestrates the various middleware layers
//! (routing, authentication, error handling, metadata, etc.) into a cohesive `tower::Service`.
//!
//! ## Scope
//! This module defines:
//! -   `Gateway`: A builder for configuring the runtime.
//! -   `RouterService`: The core service responsible for routing and authentication.
//! -   `UnescapingMode`: Configuration for path unescaping.
//! -   Type aliases for various handler callbacks (`ErrorHandler`, `AuthVerifier`, etc.).
//!
//! ## Middleware Stack
//! The `Gateway::into_service()` method constructs a `tower::Service` with the following layer order
//! (outer to inner):
//! 1.  **Tracing**: Request/Response tracing.
//! 2.  **Metrics**: Request duration and status recording.
//! 3.  **Error Handling**: Catches errors from inner layers and converts them to HTTP responses.
//! 4.  **Response Modifiers**: modifying the response before sending it back.
//! 5.  **Headers**: Filtering/Transforming incoming and outgoing headers.
//! 6.  **Metadata**: Extracting and injecting metadata (e.g., from headers or annotators).
//! 7.  **RouterService**: The core logic (Path matching -> Auth -> Dispatch).
//!
//! ## Position in the Architecture
//! The `Gateway` is the glue that binds the `Router` (generated code registry) with the
//! runtime features (handlers, defaults). The resulting service is typically passed to
//! an HTTP server (like `hyper`).

use crate::alloc::boxed::Box;
use crate::alloc::string::String;
use crate::alloc::sync::Arc;
use crate::alloc::vec::Vec;
use crate::defaults;
use crate::layers::{
    error::ErrorLayer, headers::HeaderLayer, metadata::MetadataLayer, response::ResponseLayer,
};
use crate::metadata::MetadataForwardingConfig;
use crate::router::{RouteMetadata, Router};
use crate::{GatewayError, GatewayRequest, GatewayResponse, GatewayResult};
use core::task::{Context, Poll};
use core::time::Duration;
use percent_encoding::percent_decode_str;
use std::future::Future;
use std::pin::Pin;
use tonic::metadata::MetadataMap;
use tower::{Service, ServiceBuilder};

/// A handler for converting errors into HTTP responses.
pub type ErrorHandler = Arc<dyn Fn(&GatewayRequest, GatewayError) -> GatewayResponse + Send + Sync>;

/// A handler for annotating requests with metadata.
pub type MetadataAnnotator = Arc<dyn Fn(&GatewayRequest) -> MetadataMap + Send + Sync>;

/// A handler for modifying HTTP responses before they are sent.
pub type ResponseModifier = Arc<dyn Fn(&GatewayRequest, &mut GatewayResponse) + Send + Sync>;

/// A handler for matching and transforming headers.
pub type HeaderMatcher = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// A handler for verifying authentication requirements.
pub type AuthVerifier =
    Arc<dyn Fn(&GatewayRequest, &RouteMetadata) -> Result<(), GatewayError> + Send + Sync>;

/// A handler for recording metrics.
pub type MetricsRecorder = Arc<dyn Fn(&GatewayRequest, &GatewayResult, Duration) + Send + Sync>;

/// A handler for tracing start. Returns an opaque token (TraceContext) to be passed to end.
pub type TracingStartHandler =
    Arc<dyn Fn(&GatewayRequest) -> Box<dyn core::any::Any + Send> + Send + Sync>;

/// A handler for tracing end.
pub type TracingEndHandler =
    Arc<dyn Fn(Box<dyn core::any::Any + Send>, &GatewayResult) + Send + Sync>;

/// Configuration for unescaping path parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnescapingMode {
    /// Unescape all characters using URL decoding.
    AllCharacters,
    /// Default behavior (no unescaping).
    Default,
}

/// The core service that handles routing and authentication logic.
///
/// This service matches the request path against the `Router` and executes the configured
/// `AuthVerifier`. If successful, it dispatches the request to the matched service.
#[derive(Clone)]
struct RouterService<S> {
    router: Router<S>,
    auth_verifier: Option<AuthVerifier>,
    unescaping_mode: UnescapingMode,
}

impl<S> Service<GatewayRequest> for RouterService<S>
where
    S: Service<GatewayRequest, Response = GatewayResponse, Error = GatewayError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = GatewayResponse;
    type Error = GatewayError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: GatewayRequest) -> Self::Future {
        let mut path = req.uri().path().to_string();

        // Handle path unescaping if configured
        match self.unescaping_mode {
            UnescapingMode::AllCharacters => {
                if let Ok(decoded) = percent_decode_str(&path).decode_utf8() {
                    path = decoded.to_string();
                }
            }
            UnescapingMode::Default => {}
        }

        let method = req.method().clone();

        // Match request against registered routes
        let match_result = self.router.match_request(&method, &path);

        if let Some((service, params, metadata)) = match_result {
            // Auth Verification Phase
            if let Some(verifier) = &self.auth_verifier {
                // Check authentication requirements
                match verifier(&req, metadata) {
                    Ok(_) => {}
                    Err(e) => return Box::pin(async move { Err(e) }),
                }
            }

            // Store captured path parameters for use by the service (handlers)
            req.extensions_mut().insert(params);

            let mut service = service.clone();
            Box::pin(async move { service.call(req).await })
        } else {
            // No route matched
            Box::pin(async move { Err(GatewayError::NotFound) })
        }
    }
}

/// A generic layer for recording metrics around the inner service execution.
#[derive(Clone)]
struct MetricsLayer<S> {
    inner: S,
    recorder: Option<MetricsRecorder>,
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

/// A generic layer for wrapping execution with tracing start/end hooks.
#[derive(Clone)]
struct TraceLayer<S> {
    inner: S,
    start: Option<TracingStartHandler>,
    end: Option<TracingEndHandler>,
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
        let token = if let Some(start) = &self.start {
            Some(start(&req))
        } else {
            None
        };

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

/// A builder and configuration struct for the Gateway runtime.
///
/// Wraps a `Router` and allows attaching various handlers and configuration options.
/// The `into_service()` method consumes this builder to produce the final `Service`.
pub struct Gateway<S> {
    router: Router<S>,
    error_handler: Option<ErrorHandler>,
    metadata_annotators: Vec<MetadataAnnotator>,
    response_modifiers: Vec<ResponseModifier>,
    incoming_header_matcher: Option<HeaderMatcher>,
    outgoing_header_matcher: Option<HeaderMatcher>,
    unescaping_mode: UnescapingMode,

    auth_verifier: Option<AuthVerifier>,
    metrics_recorder: Option<MetricsRecorder>,
    tracing_start: Option<TracingStartHandler>,
    tracing_end: Option<TracingEndHandler>,

    metadata_config: MetadataForwardingConfig,
}

impl<S> Gateway<S> {
    /// Creates a new `Gateway` wrapping the given `Router` and initialized with secure defaults.
    pub fn new(router: Router<S>) -> Self {
        Self {
            router,
            error_handler: Some(Arc::new(defaults::default_error_handler)),
            metadata_annotators: vec![Arc::new(defaults::default_metadata_annotator)],
            response_modifiers: vec![Arc::new(defaults::default_response_modifier)],
            incoming_header_matcher: Some(Arc::new(defaults::default_incoming_header_matcher)),
            outgoing_header_matcher: Some(Arc::new(defaults::default_outgoing_header_matcher)),
            unescaping_mode: UnescapingMode::Default,
            auth_verifier: None, // No default auth verifier, must be explicitly set if needed.
            metrics_recorder: None,
            tracing_start: None,
            tracing_end: None,
            metadata_config: MetadataForwardingConfig::default(),
        }
    }

    /// Sets the custom error handler.
    pub fn with_error_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&GatewayRequest, GatewayError) -> GatewayResponse + Send + Sync + 'static,
    {
        self.error_handler = Some(Arc::new(handler));
        self
    }

    /// Adds a metadata annotator.
    pub fn with_metadata<F>(mut self, annotator: F) -> Self
    where
        F: Fn(&GatewayRequest) -> MetadataMap + Send + Sync + 'static,
    {
        self.metadata_annotators.push(Arc::new(annotator));
        self
    }

    /// Adds a response modifier.
    pub fn with_response_modifier<F>(mut self, modifier: F) -> Self
    where
        F: Fn(&GatewayRequest, &mut GatewayResponse) + Send + Sync + 'static,
    {
        self.response_modifiers.push(Arc::new(modifier));
        self
    }

    /// Sets the incoming header matcher.
    pub fn with_incoming_header_matcher<F>(mut self, matcher: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.incoming_header_matcher = Some(Arc::new(matcher));
        self
    }

    /// Sets the outgoing header matcher.
    pub fn with_outgoing_header_matcher<F>(mut self, matcher: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.outgoing_header_matcher = Some(Arc::new(matcher));
        self
    }

    /// Sets the unescaping mode.
    pub fn with_unescaping_mode(mut self, mode: UnescapingMode) -> Self {
        self.unescaping_mode = mode;
        self
    }

    /// Sets the authentication verifier.
    pub fn with_auth_verifier<F>(mut self, verifier: F) -> Self
    where
        F: Fn(&GatewayRequest, &RouteMetadata) -> Result<(), GatewayError> + Send + Sync + 'static,
    {
        self.auth_verifier = Some(Arc::new(verifier));
        self
    }

    /// Sets the metrics recorder.
    pub fn with_metrics_recorder<F>(mut self, recorder: F) -> Self
    where
        F: Fn(&GatewayRequest, &GatewayResult, Duration) + Send + Sync + 'static,
    {
        self.metrics_recorder = Some(Arc::new(recorder));
        self
    }

    /// Sets tracing handlers.
    pub fn with_tracing<Start, End>(mut self, start: Start, end: End) -> Self
    where
        Start: Fn(&GatewayRequest) -> Box<dyn core::any::Any + Send> + Send + Sync + 'static,
        End: Fn(Box<dyn core::any::Any + Send>, &GatewayResult) + Send + Sync + 'static,
    {
        self.tracing_start = Some(Arc::new(start));
        self.tracing_end = Some(Arc::new(end));
        self
    }

    /// Sets the metadata forwarding configuration.
    pub fn with_metadata_config(mut self, config: MetadataForwardingConfig) -> Self {
        self.metadata_config = config;
        self
    }

    /// Returns a reference to the metadata configuration.
    pub fn metadata_config(&self) -> &MetadataForwardingConfig {
        &self.metadata_config
    }
}

impl<S> Gateway<S>
where
    S: Service<GatewayRequest, Response = GatewayResponse, Error = GatewayError>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    /// Consumes the Gateway configuration and returns a constructed `tower::BoxCloneService`.
    ///
    /// This method assembles the complete middleware stack around the router.
    pub fn into_service(
        self,
    ) -> tower::util::BoxCloneService<GatewayRequest, GatewayResponse, GatewayError> {
        let router_service = RouterService {
            router: self.router,
            auth_verifier: self.auth_verifier.clone(),
            unescaping_mode: self.unescaping_mode,
        };

        let service = ServiceBuilder::new()
            .layer_fn(|inner| TraceLayer {
                inner,
                start: self.tracing_start.clone(),
                end: self.tracing_end.clone(),
            })
            .layer_fn(|inner| MetricsLayer {
                inner,
                recorder: self.metrics_recorder.clone(),
            })
            .layer_fn(|inner| ErrorLayer::new(inner, self.error_handler.clone()))
            .layer_fn(|inner| ResponseLayer::new(inner, self.response_modifiers.clone()))
            .layer_fn(|inner| {
                HeaderLayer::new(
                    inner,
                    self.incoming_header_matcher.clone(),
                    self.outgoing_header_matcher.clone(),
                )
            })
            .layer_fn(|inner| {
                MetadataLayer::new(
                    inner,
                    self.metadata_annotators.clone(),
                    self.metadata_config.clone(),
                )
            })
            .service(router_service);

        tower::util::BoxCloneService::new(service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::string::ToString;
    use crate::router::{AuthConfig, AuthLocation, RouteMetadata, Router};
    use gateway_internal::path_template::{Op, OpCode, Pattern};
    use http::StatusCode;
    use http_body_util::BodyExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tower::util::BoxCloneService;

    fn test_pattern() -> Pattern {
        Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["test".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        }
    }

    fn make_router() -> Router<BoxCloneService<GatewayRequest, GatewayResponse, GatewayError>> {
        let mut router = Router::new();
        let service = tower::service_fn(|req: GatewayRequest| async move {
            let mut resp = http::Response::builder().status(StatusCode::OK);
            if let Some(val) = req.headers().get("x-bar") {
                resp = resp.header("x-echo-bar", val);
            }
            if let Some(md) = req.extensions().get::<MetadataMap>() {
                if let Some(val) = md.get("test-key") {
                    resp = resp.header("x-meta-echo", val.to_str().unwrap());
                }
            }
            Ok(resp
                .body(http_body_util::BodyExt::boxed_unsync(
                    http_body_util::Full::new(crate::bytes::Bytes::from("ok"))
                        .map_err(|_| unreachable!()),
                ))
                .unwrap())
        });
        crate::router::route(
            &mut router,
            http::Method::GET,
            test_pattern(),
            BoxCloneService::new(service),
        );
        router
    }

    #[tokio::test]
    async fn test_gateway_metrics() {
        let router = make_router();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();

        let gateway = Gateway::new(router).with_metrics_recorder(move |_, res, dur| {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            assert!(res.is_ok());
            assert!(dur.as_nanos() > 0);
        });

        let mut service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let _ = service.call(req).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_gateway_tracing() {
        let router = make_router();
        let trace_val = Arc::new(AtomicUsize::new(0));
        let tv1 = trace_val.clone();
        let tv2 = trace_val.clone();

        let gateway = Gateway::new(router).with_tracing(
            move |_| {
                tv1.fetch_add(1, Ordering::SeqCst);
                Box::new(123u32)
            },
            move |token, _| {
                let val = *token.downcast::<u32>().unwrap();
                assert_eq!(val, 123);
                tv2.fetch_add(1, Ordering::SeqCst);
            },
        );

        let mut service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let _ = service.call(req).await;
        assert_eq!(trace_val.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_gateway_auth_verifier_success() {
        let mut router: Router<BoxCloneService<GatewayRequest, GatewayResponse, GatewayError>> =
            Router::new();
        let service = tower::service_fn(|_| async {
            Ok(http::Response::new(BodyExt::boxed_unsync(
                http_body_util::Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()),
            )))
        });

        let meta = RouteMetadata {
            auth_required: Some(AuthConfig {
                scheme: "ApiKey".to_string(),
                location: AuthLocation::Header,
                name: "X-Key".to_string(),
            }),
        };
        crate::router::route_with_metadata(
            &mut router,
            http::Method::GET,
            test_pattern(),
            BoxCloneService::new(service),
            meta,
        );

        let gateway = Gateway::new(router).with_auth_verifier(|req, meta| {
            if let Some(auth) = &meta.auth_required {
                if auth.location == AuthLocation::Header {
                    if req.headers().contains_key(&auth.name) {
                        return Ok(());
                    }
                }
            }
            Err(GatewayError::Upstream(tonic::Status::unauthenticated(
                "missing key",
            )))
        });

        let mut service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .header("X-Key", "secret")
            .body(Vec::new())
            .unwrap();
        let resp = service.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_gateway_auth_verifier_fail() {
        let mut router: Router<BoxCloneService<GatewayRequest, GatewayResponse, GatewayError>> =
            Router::new();
        let service = tower::service_fn(|_| async {
            Ok(http::Response::new(BodyExt::boxed_unsync(
                http_body_util::Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()),
            )))
        });

        let meta = RouteMetadata {
            auth_required: Some(AuthConfig {
                scheme: "ApiKey".to_string(),
                location: AuthLocation::Header,
                name: "X-Key".to_string(),
            }),
        };
        crate::router::route_with_metadata(
            &mut router,
            http::Method::GET,
            test_pattern(),
            BoxCloneService::new(service),
            meta,
        );

        // Error handler needed to map verify error to response
        let gateway = Gateway::new(router)
            .with_auth_verifier(|_, _| {
                Err(GatewayError::Upstream(tonic::Status::unauthenticated(
                    "fail",
                )))
            })
            .with_error_handler(|_, _| {
                http::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(BodyExt::boxed_unsync(
                        http_body_util::Full::new(crate::bytes::Bytes::new())
                            .map_err(|_| unreachable!()),
                    ))
                    .unwrap()
            });

        let mut service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let resp = service.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
