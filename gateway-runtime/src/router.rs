//! # Router
//!
//! ## Purpose
//! Manages the routing of incoming HTTP requests to registered services based on path patterns.
//! This module acts as the request dispatcher, mapping HTTP method and path combinations
//! to specific handlers (services).
//!
//! ## Scope
//! This module defines:
//! -   `Router`: The primary registry and dispatching struct.
//! -   `route`: A helper function to register services with the router.
//! -   Request matching logic that delegates to the pattern matching engine.
//!
//! ## Position in the Architecture
//! The `Router` is instantiated by the user's application server. Generated code populates
//! it with service handlers. During runtime, the HTTP server component uses the `Router`
//! to identify which service should handle a given request.
//!
//! ## Design Constraints
//! -   **`no_std` Compatibility**: Uses `BTreeMap` for storage to avoid dependency on the standard library's `HashMap`.
//! -   **Generic Service Type**: The `Router` is generic over `S`, allowing it to store any type of service (e.g., `tower::Service`, `Box<dyn Service>`).

use crate::pattern::route_matcher;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use gateway_internal::path_template::Pattern;
use http::Method;

/// The main entry point for the gRPC gateway routing.
///
/// Stores a mapping of HTTP methods to a list of route entries. Each entry contains
/// a path pattern and the associated service.
///
/// # Type Parameters
/// *   `S`: The type of service stored in the router. This allows for flexibility in
///     the underlying service representation (e.g., boxed services, function pointers).
pub struct Router<S> {
    /// Maps HTTP method strings (e.g., "GET") to a list of routes.
    routes: BTreeMap<String, Vec<RouteEntry<S>>>,
}

/// Represents a single route registration.
struct RouteEntry<S> {
    /// The compiled path pattern for matching.
    pattern: Pattern,
    /// The service responsible for handling requests matching this pattern.
    service: S,
}

impl<S> Router<S> {
    /// Creates a new, empty `Router`.
    ///
    /// # Returns
    /// A new `Router` instance.
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
    /// -   `BTreeMap<String, String>`: A map of captured path variables (e.g., matching `{id}` in a path).
    ///
    /// Returns `None` if no route matches the method and path.
    pub fn match_request(
        &self,
        method: &Method,
        path: &str,
    ) -> Option<(&S, BTreeMap<String, String>)> {
        if let Some(entries) = self.routes.get(method.as_str()) {
            for entry in entries {
                if let Some(captured) = route_matcher(&entry.pattern, path) {
                    return Some((&entry.service, captured));
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

/// Registers a service with the router.
///
/// This function adds a new route to the `Router` for a specific HTTP method and path pattern.
///
/// # Parameters
/// *   `router`: The `Router` instance to modify.
/// *   `method`: The HTTP method for this route.
/// *   `pattern`: The compiled path pattern.
/// *   `service`: The service to register.
///
/// # Type Parameters
/// *   `S`: The service type stored in the router.
/// *   `C`: The type of the service being registered. It must be convertible into `S`.
pub fn route<S, C>(router: &mut Router<S>, method: Method, pattern: Pattern, service: C)
where
    C: Into<S>,
{
    router
        .routes
        .entry(method.to_string())
        .or_default()
        .push(RouteEntry {
            pattern,
            service: service.into(),
        });
}
