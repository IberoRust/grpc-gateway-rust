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
use crate::alloc::sync::Arc;
use crate::alloc::vec::Vec;
use crate::metadata::MetadataForwardingConfig;
use crate::GatewayRequest;
use core::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tonic::metadata::MetadataMap;
use tower::Service;

/// A handler for annotating requests with metadata.
///
/// Functions of this type analyze the incoming [GatewayRequest] and return a `tonic::metadata::MetadataMap`
/// containing key-value pairs to be associated with the gRPC context (e.g., `x-request-id`).
pub type MetadataAnnotator = Arc<dyn Fn(&GatewayRequest) -> MetadataMap + Send + Sync>;

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
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GatewayError;
    use crate::GatewayResponse;
    use http_body_util::BodyExt;
    use http_body_util::Full;

    #[tokio::test]
    async fn test_metadata_layer_annotates() {
        let annotator: MetadataAnnotator = Arc::new(|_| {
            let mut map = MetadataMap::new();
            map.insert("x-test", "value".parse().unwrap());
            map
        });

        let service = tower::service_fn(|req: GatewayRequest| async move {
            let md = req.extensions().get::<MetadataMap>().unwrap();
            assert_eq!(md.get("x-test").unwrap(), "value");
            Ok::<GatewayResponse, GatewayError>(http::Response::new(
                BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))
            ))
        });

        let mut layer = MetadataLayer::new(service, vec![annotator], MetadataForwardingConfig::default());
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
    }

    #[tokio::test]
    async fn test_metadata_layer_merges() {
        let a1: MetadataAnnotator = Arc::new(|_| {
            let mut map = MetadataMap::new();
            map.insert("k1", "v1".parse().unwrap());
            map
        });
        let a2: MetadataAnnotator = Arc::new(|_| {
            let mut map = MetadataMap::new();
            map.insert("k2", "v2".parse().unwrap());
            map
        });

        let service = tower::service_fn(|req: GatewayRequest| async move {
            let md = req.extensions().get::<MetadataMap>().unwrap();
            assert_eq!(md.get("k1").unwrap(), "v1");
            assert_eq!(md.get("k2").unwrap(), "v2");
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = MetadataLayer::new(service, vec![a1, a2], MetadataForwardingConfig::default());
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
    }

    #[tokio::test]
    async fn test_metadata_layer_injects_config() {
        let config = MetadataForwardingConfig::default(); // Assume default allows something or is empty
        let service = tower::service_fn(|req: GatewayRequest| async move {
            assert!(req.extensions().get::<MetadataForwardingConfig>().is_some());
            Ok::<GatewayResponse, GatewayError>(http::Response::new(BodyExt::boxed_unsync(Full::new(crate::bytes::Bytes::new()).map_err(|_| unreachable!()))))
        });

        let mut layer = MetadataLayer::new(service, vec![], config);
        let req = http::Request::builder().body(crate::alloc::vec::Vec::new()).unwrap();

        layer.call(req).await.unwrap();
    }
}
