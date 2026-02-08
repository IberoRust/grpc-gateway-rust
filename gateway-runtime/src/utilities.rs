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
        codec.decode(&body)
    }
}

/// Parses the request body into a Protobuf message (no_std fallback).
#[cfg(not(feature = "std"))]
pub async fn parse_body<T, C>(
    _headers: &http::HeaderMap,
    body: alloc::vec::Vec<u8>,
    codec: &C,
) -> Result<T, crate::errors::GatewayError>
where
    T: prost::Message + Default + serde::de::DeserializeOwned,
    C: crate::codec::Codec,
{
    codec.decode(&body)
}
