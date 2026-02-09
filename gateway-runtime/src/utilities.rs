//! # Utilities
//!
//! ## Purpose
//! Shared helper functions and types that don't fit neatly into other modules.
//!
//! ## Scope
//! This module defines:
//! -   `SyncService`: A thread-safe wrapper for services (enabled with `std` feature).
//! -   `parse_path_param`: Helper for parsing path parameters.
//! -   `parse_body`: Helper for parsing request bodies (handles multipart).

#[allow(unused_imports)]
use crate::alloc;
use core::str::FromStr;

#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(feature = "std")]
use crate::codec::Codec;
#[cfg(feature = "std")]
use crate::errors::GatewayError;
#[cfg(feature = "std")]
use bytes::Bytes;
#[cfg(feature = "std")]
use prost::Message;
#[cfg(feature = "std")]
use serde::de::DeserializeOwned;
#[cfg(feature = "std")]
use std::task::{Context, Poll};
#[cfg(feature = "std")]
use tower::Service;

/// A thread-safe wrapper for a service, enabling sharing across threads.
///
/// This struct wraps a service in a `Mutex`, making it `Sync` (provided the inner service is `Send`).
/// It is particularly useful for wrapping `BoxCloneService` which is `!Sync` by default, allowing
/// it to be stored in a `Router` that is shared via `Arc`.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct SyncService<S>(pub Mutex<S>);

#[cfg(feature = "std")]
impl<S> SyncService<S> {
    /// Creates a new `SyncService` wrapping the given service.
    pub fn new(service: S) -> Self {
        Self(Mutex::new(service))
    }

    /// Acquires a lock on the inner service.
    ///
    /// # Panics
    /// Panics if the lock is poisoned.
    pub fn get(&self) -> std::sync::MutexGuard<'_, S> {
        self.0.lock().unwrap()
    }
}

#[cfg(feature = "std")]
impl<S> From<S> for SyncService<S> {
    fn from(service: S) -> Self {
        Self::new(service)
    }
}

#[cfg(feature = "std")]
impl<S: Clone> Clone for SyncService<S> {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.get().clone()))
    }
}

#[cfg(feature = "std")]
impl<S, Request> Service<Request> for SyncService<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.get().poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.get().call(req)
    }
}

/// Parses a path parameter string into a target type.
///
/// # Parameters
/// *   `value`: The string value of the path parameter.
///
/// # Returns
/// A `Result` containing the parsed value or the parsing error.
pub fn parse_path_param<T: FromStr>(value: &str) -> Result<T, T::Err> {
    value.parse()
}

/// Parses the request body into a Protobuf message.
///
/// This function handles:
/// - `application/json`: Decodes using the provided codec.
/// - `multipart/form-data`: Parses multipart parts and maps them to the message fields.
///
/// # Parameters
/// *   `headers`: The request headers.
/// *   `body`: The request body as bytes.
/// *   `codec`: The codec to use for decoding.
///
/// # Returns
/// A `Result` containing the parsed message or a `GatewayError`.
#[cfg(feature = "std")]
pub async fn parse_body<T, C>(
    headers: &http::HeaderMap,
    body: alloc::vec::Vec<u8>,
    codec: &C,
) -> Result<T, GatewayError>
where
    T: Message + Default + DeserializeOwned,
    C: Codec,
{
    let content_type = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if content_type.starts_with("multipart/form-data") {
        let boundary = multer::parse_boundary(content_type)
            .map_err(|e| GatewayError::Encoding(Box::new(e)))?;

        let stream = futures::stream::iter(vec![Ok::<Bytes, multer::Error>(Bytes::from(body))]);
        let mut multipart = multer::Multipart::new(stream, boundary);
        let mut map = serde_json::Map::new();

        while let Some(mut field) = multipart
            .next_field()
            .await
            .map_err(|e| GatewayError::Encoding(Box::new(e)))?
        {
            let name = field.name().map(|s| s.to_string());
            if let Some(name) = name {
                let mut data = alloc::vec::Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|e| GatewayError::Encoding(Box::new(e)))?
                {
                    data.extend_from_slice(&chunk);
                }

                // Heuristic: If filename exists, treat as bytes (array of numbers).
                // If not, try string.
                if field.file_name().is_some() {
                    let arr: alloc::vec::Vec<serde_json::Value> = data
                        .into_iter()
                        .map(|b| serde_json::Value::Number(serde_json::Number::from(b)))
                        .collect();
                    map.insert(name, serde_json::Value::Array(arr));
                } else {
                    if let Ok(s) = String::from_utf8(data.clone()) {
                        map.insert(name, serde_json::Value::String(s));
                    } else {
                        let arr: alloc::vec::Vec<serde_json::Value> = data
                            .into_iter()
                            .map(|b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect();
                        map.insert(name, serde_json::Value::Array(arr));
                    }
                }
            }
        }

        let value = serde_json::Value::Object(map);
        serde_json::from_value(value).map_err(|e| GatewayError::Encoding(Box::new(e)))
    } else {
        let content_type = if content_type.is_empty() {
            None
        } else {
            Some(content_type)
        };
        codec.decode(&body, content_type)
    }
}

/// Parses the request body into a Protobuf message (no_std fallback).
#[cfg(not(feature = "std"))]
pub async fn parse_body<T, C>(
    headers: &http::HeaderMap,
    body: alloc::vec::Vec<u8>,
    codec: &C,
) -> Result<T, crate::errors::GatewayError>
where
    T: prost::Message + Default + serde::de::DeserializeOwned,
    C: crate::codec::Codec,
{
    let content_type = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok());
    codec.decode(&body, content_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    #[test]
    fn test_parse_path_param_string() {
        let res: Result<String, _> = parse_path_param("abc");
        assert_eq!(res.unwrap(), "abc");
    }

    #[test]
    fn test_parse_path_param_int() {
        let res: Result<i32, _> = parse_path_param("123");
        assert_eq!(res.unwrap(), 123);
    }

    #[test]
    fn test_parse_path_param_invalid() {
        let res: Result<i32, _> = parse_path_param("abc");
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_parse_body_json() {
        let body = r#"{"foo": "bar"}"#.as_bytes().to_vec();
        let headers = HeaderMap::new();
        // Requires codec implementation.
        struct MockCodec;
        impl Codec for MockCodec {
            fn encode<T: Message + serde::Serialize>(
                &self,
                _item: &T,
                _buf: Option<&str>,
            ) -> Result<crate::bytes::Bytes, GatewayError> {
                unimplemented!()
            }
            fn decode<T: Message + Default + serde::de::DeserializeOwned>(
                &self,
                buf: &[u8],
                _content_type: Option<&str>,
            ) -> Result<T, GatewayError> {
                let s = String::from_utf8(buf.to_vec()).unwrap();
                if s.contains("foo") {
                    Ok(T::default())
                } else {
                    Err(GatewayError::Encoding(Box::new(std::fmt::Error)))
                }
            }
            fn encoder_content_type(&self, _accept: Option<&str>) -> String {
                "application/json".to_string()
            }
        }

        #[derive(serde::Deserialize)]
        struct Dummy {
            #[serde(default)]
            foo: String,
        }
        // Manual impls to avoid conflicts
        impl Default for Dummy {
            fn default() -> Self {
                Self { foo: String::new() }
            }
        }
        impl prost::Message for Dummy {
            fn encode_raw(&self, _buf: &mut impl bytes::BufMut) {}
            fn merge_field(
                &mut self,
                _tag: u32,
                _wire_type: prost::encoding::WireType,
                _buf: &mut impl bytes::Buf,
                _ctx: prost::encoding::DecodeContext,
            ) -> Result<(), prost::DecodeError> {
                Ok(())
            }
            fn encoded_len(&self) -> usize {
                0
            }
            fn clear(&mut self) {
                self.foo.clear();
            }
        }

        // Codec decode is called.
        let codec = MockCodec;
        let res: Result<Dummy, _> = parse_body(&headers, body, &codec).await;
        assert!(res.is_ok());
    }

    // SyncService test
    #[test]
    fn test_sync_service() {
        let val = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let val_clone = val.clone();

        // Mock service
        struct S(std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl Service<()> for S {
            type Response = ();
            type Error = ();
            type Future = std::future::Ready<Result<(), ()>>;
            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), ()>> {
                Poll::Ready(Ok(()))
            }
            fn call(&mut self, _req: ()) -> Self::Future {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::future::ready(Ok(()))
            }
        }
        impl Clone for S {
            fn clone(&self) -> Self {
                S(self.0.clone())
            }
        }

        let s = S(val);
        // SyncService wraps S.
        // We verify Clone works and call works.
        let mut sync_s = SyncService::new(s);
        let mut cloned = sync_s.clone();

        let _ = sync_s.call(());
        let _ = cloned.call(());

        assert_eq!(val_clone.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
