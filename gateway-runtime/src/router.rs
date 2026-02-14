//! # Router
//!
//! ## Purpose
//! The `Router` module provides the core mechanism for dispatching incoming HTTP requests to
//! the appropriate gRPC service handlers based on path patterns and HTTP methods.
//!
//! ## Overview
//! It maintains a registry of routes, where each route consists of:
//! -   An HTTP Method (e.g., GET, POST).
//! -   A compiled [Pattern] (from `gateway_internal::path_template`).
//! -   A service handler (`S`) responsible for processing the request.
//! -   [RouteMetadata], containing additional configuration like authentication requirements.
//!
//! ## Matching Logic
//! When `match_request` is called, the router iterates through the registered patterns for the
//! given HTTP method. It uses the `gateway_internal` matching engine to determine if the
//! request path matches a pattern. If a match is found, it returns the service, any captured
//! path variables, and the route metadata.
//!
//! ## Usage
//! The router is typically populated by generated code calling `route` or `route_with_metadata`.
//! At runtime, it is wrapped by the `Gateway` service.

use crate::alloc::sync::Arc;
use crate::pattern::route_matcher;
use crate::{GatewayError, GatewayRequest};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use gateway_internal::path_template::Pattern;
use http::Method;

/// Metadata associated with a route configuration.
///
/// This struct holds static configuration derived from the `.proto` options, such as
/// authentication requirements (e.g., `google.api.http` security rules).
#[derive(Debug, Clone, Default)]
pub struct RouteMetadata {
    /// Configuration for API Key authentication, if required by the route.
    pub auth_required: Option<AuthConfig>,
}

/// Configuration for API Key authentication.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// The authentication scheme (e.g., "ApiKey").
    pub scheme: String,
    /// The location of the API key in the request.
    pub location: AuthLocation,
    /// The name of the header, query parameter, or cookie.
    pub name: String,
}

/// The location of the authentication credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthLocation {
    /// Credential is in an HTTP header.
    Header,
    /// Credential is in the URL query string.
    Query,
    /// Credential is in a cookie.
    Cookie,
}

/// A handler for verifying authentication requirements.
///
/// Implementations check if the request satisfies the security rules defined in the [RouteMetadata].
/// If validation fails, it should return a [GatewayError] (typically `Unauthenticated` or `PermissionDenied`).
pub type AuthVerifier =
    Arc<dyn Fn(&GatewayRequest, &RouteMetadata) -> Result<(), GatewayError> + Send + Sync>;

/// The request dispatcher.
///
/// Maps (HTTP Method) -> List of (Pattern, Service, Metadata).
///
/// # Type Parameters
/// *   `S`: The type of the service handler. This is typically `BoxCloneService` or similar.
#[derive(Clone)]
pub struct Router<S> {
    routes: BTreeMap<String, Vec<RouteEntry<S>>>,
}

/// A single entry in the routing table.
#[derive(Clone)]
struct RouteEntry<S> {
    pattern: Pattern,
    service: S,
    metadata: RouteMetadata,
}

impl<S> Router<S> {
    /// Creates a new, empty `Router`.
    pub fn new() -> Self {
        Self {
            routes: BTreeMap::new(),
        }
    }

    /// Matches an incoming request against registered routes.
    ///
    /// # Parameters
    /// *   `method`: The HTTP method of the request.
    /// *   `path`: The request path.
    ///
    /// # Returns
    /// An `Option` containing a tuple if a match is found:
    /// -   `&S`: A reference to the matched service.
    /// -   `BTreeMap<String, String>`: A map of captured path variables (e.g., `id` from `/users/{id}`).
    /// -   `&RouteMetadata`: Metadata associated with the matched route.
    pub fn match_request(
        &self,
        method: &Method,
        path: &str,
    ) -> Option<(&S, BTreeMap<String, String>, &RouteMetadata)> {
        if let Some(entries) = self.routes.get(method.as_str()) {
            for entry in entries {
                if let Some(captured) = route_matcher(&entry.pattern, path) {
                    return Some((&entry.service, captured, &entry.metadata));
                }
            }
        }
        None
    }
}

impl<S> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// Registers a service with the router using default metadata.
///
/// # Parameters
/// *   `router`: The router instance.
/// *   `method`: HTTP method.
/// *   `pattern`: Path pattern.
/// *   `service`: The service handler.
pub fn route<S, C>(router: &mut Router<S>, method: Method, pattern: Pattern, service: C)
where
    C: Into<S>,
{
    route_with_metadata(router, method, pattern, service, RouteMetadata::default())
}

/// Registers a service with the router, including specific metadata.
///
/// # Parameters
/// *   `router`: The router instance.
/// *   `method`: HTTP method.
/// *   `pattern`: Path pattern.
/// *   `service`: The service handler.
/// *   `metadata`: Route-specific metadata.
pub fn route_with_metadata<S, C>(
    router: &mut Router<S>,
    method: Method,
    pattern: Pattern,
    service: C,
    metadata: RouteMetadata,
) where
    C: Into<S>,
{
    router
        .routes
        .entry(method.to_string())
        .or_default()
        .push(RouteEntry {
            pattern,
            service: service.into(),
            metadata,
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_internal::path_template::{Op, OpCode};

    #[derive(Clone)]
    struct MockService;
    impl MockService {
        fn new() -> Self {
            Self
        }
    }

    #[test]
    fn test_router_insert_and_match() {
        let mut router: Router<MockService> = Router::new();
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        route(&mut router, Method::GET, pattern, MockService::new());

        let res = router.match_request(&Method::GET, "/foo");
        assert!(res.is_some());
    }

    #[test]
    fn test_router_method_mismatch() {
        let mut router: Router<MockService> = Router::new();
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        route(&mut router, Method::GET, pattern, MockService::new());

        let res = router.match_request(&Method::POST, "/foo");
        assert!(res.is_none());
    }

    #[test]
    fn test_router_path_mismatch() {
        let mut router: Router<MockService> = Router::new();
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        route(&mut router, Method::GET, pattern, MockService::new());
        assert!(router.match_request(&Method::GET, "/bar").is_none());
    }

    #[test]
    fn test_router_multiple_routes() {
        let mut router: Router<MockService> = Router::new();
        let p1 = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        let p2 = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["bar".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };

        route(&mut router, Method::GET, p1, MockService::new());
        route(&mut router, Method::GET, p2, MockService::new());

        assert!(router.match_request(&Method::GET, "/foo").is_some());
        assert!(router.match_request(&Method::GET, "/bar").is_some());
    }

    #[test]
    fn test_router_precedence() {
        let mut router: Router<MockService> = Router::new();

        // /foo
        let p1 = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        // /{var}
        let p2 = Pattern {
            ops: vec![
                Op {
                    code: OpCode::Push,
                    operand: 0,
                },
                Op {
                    code: OpCode::Capture,
                    operand: 0,
                },
            ],
            pool: vec![],
            vars: vec!["v".to_string()],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };

        // Route specific first
        route(&mut router, Method::GET, p1, MockService::new());
        route(&mut router, Method::GET, p2, MockService::new());

        let (_s, c, _) = router.match_request(&Method::GET, "/foo").unwrap();
        assert!(c.is_empty()); // p1 has no capture. p2 has.

        let (_s, c, _) = router.match_request(&Method::GET, "/bar").unwrap();
        assert!(!c.is_empty()); // Matches p2.
    }

    #[test]
    fn test_router_metadata() {
        let mut router: Router<MockService> = Router::new();
        let pattern = Pattern {
            ops: vec![Op {
                code: OpCode::LitPush,
                operand: 0,
            }],
            pool: vec!["foo".to_string()],
            vars: vec![],
            stack_size: 1,
            tail_len: 0,
            verb: None,
        };
        let meta = RouteMetadata {
            auth_required: Some(AuthConfig {
                scheme: "ApiKey".to_string(),
                location: AuthLocation::Header,
                name: "X-API-Key".to_string(),
            }),
        };

        route_with_metadata(
            &mut router,
            Method::GET,
            pattern,
            MockService::new(),
            meta.clone(),
        );

        let (_, _, matched_meta) = router.match_request(&Method::GET, "/foo").unwrap();
        assert!(matched_meta.auth_required.is_some());
        let auth = matched_meta.auth_required.as_ref().unwrap();
        assert_eq!(auth.name, "X-API-Key");
    }
}
