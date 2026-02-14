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
//! 1.  **Response Boxing & Compression**: Ensures uniform response type.
//! 2.  **Governance**: Concurrency limits and Timeouts.
//! 3.  **Tower HTTP**: CORS, Tracing (requires `Body` trait).
//! 4.  **Adapters**: Bridges `Vec<u8>` and `http_body::Body`.
//! 5.  **Governance Body Limit**: Enforces size limits on `Vec<u8>`.
//! 6.  **Internal Layers**: Metrics, Tracing, Error Handling, etc.
//! 7.  **RouterService**: The core logic (Path matching -> Auth -> Dispatch).

use crate::alloc::boxed::Box;
use crate::alloc::string::String;
use crate::alloc::sync::Arc;
use crate::alloc::vec::Vec;
use crate::defaults;
use crate::layers::{
    body::{box_response_body, VecBody, VecBodyToVecService},
    error::{ErrorHandler, ErrorLayer},
    governance::{BodyLimitLayer, GatewayRetryPolicy, GovernanceConfig},
    headers::{HeaderLayer, HeaderMatcher},
    health::{HealthCheckConfig, HealthService},
    metadata::{MetadataAnnotator, MetadataLayer},
    metrics::{MetricsLayer, MetricsRecorder},
    response::{ResponseLayer, ResponseModifier},
    tracing::{TraceLayer, TracingEndHandler, TracingStartHandler},
};
use crate::metadata::MetadataForwardingConfig;
use crate::router::{AuthVerifier, RouteMetadata, Router};
use crate::{GatewayError, GatewayRequest, GatewayResponse, GatewayResult};
use core::task::{Context, Poll};
use core::time::Duration;
use percent_encoding::percent_decode_str;
use std::future::Future;
use std::pin::Pin;
use tonic::metadata::MetadataMap;
use tower::{Service, ServiceBuilder};
use tower::util::{MapErrLayer, MapRequestLayer, MapResponseLayer};
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer as HttpTraceLayer;

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

// Helper to map GatewayError to BoxError in a Clone-safe way (function pointer)
fn gateway_error_to_box_error(e: GatewayError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
}

// Helper to lift GatewayError to BoxError (inverse mapping for Buffer)
fn box_error_to_gateway_error(e: Box<dyn std::error::Error + Send + Sync>) -> GatewayError {
    GatewayError::Custom(
        http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Governance error: {}", e),
    )
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

    cors_layer: Option<CorsLayer>,
    compression_layer: Option<CompressionLayer>,
    governance_config: GovernanceConfig,
    health_check_config: Option<HealthCheckConfig>,

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
            cors_layer: None,
            compression_layer: None,
            governance_config: GovernanceConfig::default(),
            health_check_config: None,
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
    #[deprecated(note = "Use tower-http TraceLayer or metrics crate instead.")]
    pub fn with_metrics_recorder<F>(mut self, recorder: F) -> Self
    where
        F: Fn(&GatewayRequest, &GatewayResult, Duration) + Send + Sync + 'static,
    {
        self.metrics_recorder = Some(Arc::new(recorder));
        self
    }

    /// Sets the CORS layer.
    pub fn with_cors(mut self, layer: CorsLayer) -> Self {
        self.cors_layer = Some(layer);
        self
    }

    /// Sets the compression layer.
    pub fn with_compression(mut self, layer: CompressionLayer) -> Self {
        self.compression_layer = Some(layer);
        self
    }

    /// Sets tracing handlers.
    #[deprecated(note = "Use tower-http tracing via `tracing` crate instead.")]
    pub fn with_tracing<Start, End>(mut self, start: Start, end: End) -> Self
    where
        Start: Fn(&GatewayRequest) -> Box<dyn core::any::Any + Send> + Send + Sync + 'static,
        End: Fn(Box<dyn core::any::Any + Send>, &GatewayResult) + Send + Sync + 'static,
    {
        self.tracing_start = Some(Arc::new(start));
        self.tracing_end = Some(Arc::new(end));
        self
    }

    /// Sets the governance configuration (limits, timeouts).
    pub fn with_governance(mut self, config: GovernanceConfig) -> Self {
        self.governance_config = config;
        self
    }

    /// Sets the health check configuration.
    pub fn with_health_check(mut self, config: HealthCheckConfig) -> Self {
        self.health_check_config = Some(config);
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

        // 1. Build the inner-most stack (Previous Generation Layers + BodyLimit + Retry)
        // These layers operate on `GatewayRequest` (i.e., `Request<Vec<u8>>`).
        // Retry must happen here because it needs to clone the request (which is cheap-ish for Vec<u8> vs streams).
        let inner_service = ServiceBuilder::new()
            .option_layer(self.governance_config.retry_count.map(|count| {
                tower::retry::RetryLayer::new(GatewayRetryPolicy::new(count))
            }))
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
            // Manually constructed layers must be added via layer_fn or wrapper struct if they implement Service directly
            .layer_fn(|inner| BodyLimitLayer::new(
                inner,
                self.governance_config.max_request_body_size,
                self.governance_config.max_response_body_size,
            ))
            .service(router_service);

        // 2. Build the middle stack (Governance + Adapters + TowerHTTP)

        // This stack handles error normalization, timeouts, concurrency, and protocol adapters.
        // We use `Buffer` to ensure the service is `Clone` and handle backpressure,
        // which solves the `Either` Clone issues with RateLimit/LoadShed.

        let governed_stack = ServiceBuilder::new()
            // Map BoxError (from Governance) back to GatewayError
            .layer(MapErrLayer::new(box_error_to_gateway_error))

            // Buffer the governance stack. This returns a service that produces BoxError.
            .layer(tower::buffer::BufferLayer::new(1024))

            // Governance Layers (LoadShed / RateLimit / Timeout / Concurrency) - Return BoxError
            // Order: LoadShed (fastest) -> RateLimit -> Concurrency -> Timeout
            .option_layer(if self.governance_config.enable_load_shedding {
                Some(tower::load_shed::LoadShedLayer::new())
            } else {
                None
            })
            .option_layer(self.governance_config.rate_limit_per_second.map(|rps| {
                tower::limit::RateLimitLayer::new(
                    rps,
                    core::time::Duration::from_secs(1)
                )
            }))
            .option_layer(self.governance_config.connection_limit.map(tower::limit::GlobalConcurrencyLimitLayer::new))
            .option_layer(self.governance_config.request_timeout.map(tower::timeout::TimeoutLayer::new))

            // Map GatewayError to BoxError (Required for Governance layers that expect standard Error trait)
            // Note: We use a function pointer to ensure the layer is Clone.
            .layer(MapErrLayer::new(gateway_error_to_box_error))

            // Adapt Request<Vec<u8>> -> Request<VecBody> for TowerHTTP
            .layer(MapRequestLayer::new(|req: GatewayRequest| {
                req.map(|v| VecBody(Some(v)))
            }))

            // Tower HTTP Layers
            .layer(HttpTraceLayer::new_for_http())
            .option_layer(self.cors_layer)

            // Adapt Request<VecBody> -> Request<Vec<u8>> for Inner Service
            .layer_fn(VecBodyToVecService::new)
            .service(inner_service);

        // 3. Health Check Layer (Outer Stack)
        // Wraps everything to ensure health checks work even if inner services are busy/limited.

        // We handle type unification by first boxing the response body, then applying health, then compression, then boxing response again if needed.
        // Actually, simple solution:
        // governed_stack returns Result<Response<UnsyncBoxBody>, GatewayError>.
        // Wait, governed_stack uses MapErr(gateway_error_to_box_error) -> VecBodyToVecService.
        // VecBodyToVecService returns Response<UnsyncBoxBody>.
        // BUT inner_service is wrapped with VecBodyToVecService.
        // The return type of `inner_service` (RouterService) is GatewayResponse (Response<UnsyncBoxBody>).
        // HttpTraceLayer wraps body in ResponseBody.
        // So `governed_stack` returns `Response<ResponseBody<...>>`.

        // We must normalize the response body BEFORE HealthService if HealthService returns GatewayResponse.
        // HealthService returns GatewayResponse (Response<UnsyncBoxBody>).
        // If it wraps `governed_stack`, then `governed_stack` must return GatewayResponse.
        // So we need MapResponse(box_response_body) inside the Health branch, or before it.

        let normalized_stack = ServiceBuilder::new()
            .layer(MapResponseLayer::new(box_response_body))
            .service(governed_stack);

        // normalized_stack returns GatewayResponse.

        let service_with_health = if let Some(config) = self.health_check_config {
            let (reporter, _) = tonic_health::server::health_reporter();
            let health_layer = tower::layer::layer_fn(move |inner| {
                 HealthService::new(inner, reporter.clone(), config.clone())
            });

            let svc = ServiceBuilder::new()
                .layer(health_layer)
                .service(normalized_stack);
            tower::util::BoxCloneService::new(svc)
        } else {
            tower::util::BoxCloneService::new(normalized_stack)
        };

        if let Some(compression) = self.compression_layer {
            let service = ServiceBuilder::new()
                .layer(MapResponseLayer::new(box_response_body)) // Box again after compression
                .layer(compression)
                .service(service_with_health);
            tower::util::BoxCloneService::new(service)
        } else {
            service_with_health
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
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
    use tower::ServiceExt;

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

        let service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let _ = service.oneshot(req).await;
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

        let service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let _ = service.oneshot(req).await;
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

        let service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .header("X-Key", "secret")
            .body(Vec::new())
            .unwrap();
        let resp = service.oneshot(req).await.unwrap();
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

        let service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let resp = service.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_gateway_cors() {
        let router = make_router();
        let cors =
            CorsLayer::new().allow_origin("http://example.com".parse::<http::HeaderValue>().unwrap());
        let gateway = Gateway::new(router).with_cors(cors);

        let service = gateway.into_service();
        let req = http::Request::builder()
            .method("OPTIONS")
            .uri("/test")
            .header("Origin", "http://example.com")
            .header("Access-Control-Request-Method", "GET")
            .body(Vec::new())
            .unwrap();

        let resp = service.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("access-control-allow-origin").unwrap(),
            "http://example.com"
        );
    }

    #[tokio::test]
    async fn test_gateway_tracing_tower() {
        let router = make_router();
        // Just verify it doesn't crash. We can't easily assert logs here without capturing subscriber.
        let gateway = Gateway::new(router); // HttpTraceLayer is default
        let service = gateway.into_service();
        let req = http::Request::builder()
            .method("GET")
            .uri("/test")
            .body(Vec::new())
            .unwrap();
        let resp = service.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
