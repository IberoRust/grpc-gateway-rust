//! # Middleware Layers
//!
//! This module contains the `tower::Service` middleware layers used by the [Gateway](crate::Gateway)
//! to compose the runtime behavior. Each layer is responsible for a specific aspect of request/response
//! processing.
//!
//! ## Available Layers
//!
//! *   **[error::ErrorLayer]**: Catches errors from inner services and converts them to HTTP responses
//!     using a configured [ErrorHandler](crate::gateway::ErrorHandler).
//! *   **[headers::HeaderLayer]**: Filters and transforms incoming and outgoing HTTP headers.
//! *   **[metadata::MetadataLayer]**: Extracts metadata from the request (e.g., via annotators) and
//!     injects it into the request extensions for downstream processing.
//! *   **[response::ResponseLayer]**: Applies modifications to the HTTP response before it is returned.

pub mod error;
pub mod headers;
pub mod metadata;
pub mod response;
