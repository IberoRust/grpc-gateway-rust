//! # Metadata Extraction Layer
//!
//! This layer orchestrates the extraction of gRPC metadata from incoming HTTP requests.
//!
//! It executes registered [MetadataAnnotator] functions to pull context (e.g., Request IDs,
//! Auth Tokens) into a `MetadataMap`. The resulting metadata is stored in the request
//! extensions, where it can be later retrieved by the generated code to populate the
//! `tonic::Request`.
//!
//! It also injects the [MetadataForwardingConfig] into the extensions, ensuring downstream
//! components respect the configured security rules.

use crate::alloc::boxed::Box;
use crate::alloc::vec::Vec;
use crate::gateway::MetadataAnnotator;
use crate::metadata::MetadataForwardingConfig;
use crate::GatewayRequest;
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service;

/// A Tower middleware that executes metadata annotators.
#[derive(Clone)]
pub struct MetadataLayer<S> {
    inner: S,
    annotators: Vec<MetadataAnnotator>,
    config: MetadataForwardingConfig,
}

impl<S> MetadataLayer<S> {
    /// Creates a new `MetadataLayer`.
    ///
    /// # Parameters
    /// *   `inner`: The inner service.
    /// *   `annotators`: A list of functions that extract metadata from the request.
    /// *   `config`: Security configuration for metadata forwarding.
    pub fn new(
        inner: S,
        annotators: Vec<MetadataAnnotator>,
        config: MetadataForwardingConfig,
    ) -> Self {
        Self {
            inner,
            annotators,
            config,
        }
    }
}

impl<S> Service<GatewayRequest> for MetadataLayer<S>
where
    S: Service<GatewayRequest>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: GatewayRequest) -> Self::Future {
        // Execute all registered annotators
        for annotator in &self.annotators {
            let metadata = annotator(&req);
            if !metadata.is_empty() {
                // Merge new metadata into existing extensions
                if let Some(existing) = req
                    .extensions_mut()
                    .get_mut::<tonic::metadata::MetadataMap>()
                {
                    for item in metadata.iter() {
                        match item {
                            tonic::metadata::KeyAndValueRef::Ascii(key, val) => {
                                existing.insert(key.clone(), val.clone());
                            }
                            tonic::metadata::KeyAndValueRef::Binary(key, val) => {
                                existing.insert_bin(key.clone(), val.clone());
                            }
                        }
                    }
                } else {
                    // Initialize if missing
                    req.extensions_mut().insert(metadata);
                }
            }
        }

        // Inject the forwarding configuration for use by `forward_metadata`
        req.extensions_mut().insert(self.config.clone());

        let fut = self.inner.call(req);
        Box::pin(async move { fut.await })
    }
}
