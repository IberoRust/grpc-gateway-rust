//! # Codec
//!
//! ## Purpose
//! Defines the interface and implementations for encoding and decoding gRPC messages
//! to and from HTTP body formats. This module abstracts the wire format details, allowing
//! the gateway to support multiple content types (e.g., JSON, Protocol Buffers).
//!
//! ## Scope
//! This module provides:
//! -   The `Codec` trait, which defines the contract for message serialization and deserialization.
//! -   `ProtobufCodec`: A concrete implementation for the binary Protocol Buffers format (`application/octet-stream`).
//! -   `JsonCodec`: A concrete implementation for the JSON format (`application/json`).
//!
//! ## Position in the Architecture
//! The `Codec` is used by the generated service registration code to unmarshal incoming HTTP request bodies
//! into gRPC request messages and to marshal gRPC response messages back into HTTP response bodies.
//!
//! ## Design Constraints
//! -   **Concurrency**: Codecs must be `Send`, `Sync`, and `'static` to allow sharing across threads.
//! -   **Statelessness**: Implementations are generally expected to be stateless.
//! -   **Serialization Support**: Relies on `serde` for JSON and `prost` for Protocol Buffers.

#[allow(unused_imports)]
use crate::alloc;
use crate::errors::GatewayError;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;
use alloc::vec::Vec;
use bytes::Bytes;
use prost::Message;
use serde::de::DeserializeOwned;

/// Defines how to encode and decode gRPC messages to/from HTTP bodies.
///
/// This trait abstracts the serialization logic, enabling the gateway to support various
/// wire formats.
pub trait Codec: Send + Sync + 'static {
    /// The Content-Type header value associated with this codec (e.g., "application/json").
    const CONTENT_TYPE: &'static str;

    /// Encodes a message into a buffer.
    ///
    /// # Parameters
    /// *   `val`: The message to encode. Must implement `prost::Message` and `serde::Serialize`.
    ///
    /// # Returns
    /// A `Result` containing the encoded bytes as `bytes::Bytes` or a `GatewayError` on failure.
    fn encode<T: Message + serde::Serialize>(&self, val: &T) -> Result<Bytes, GatewayError>;

    /// Decodes a buffer into a message.
    ///
    /// # Parameters
    /// *   `buf`: The byte slice to decode.
    ///
    /// # Returns
    /// A `Result` containing the decoded message of type `T` or a `GatewayError` on failure.
    fn decode<T: Message + Default + DeserializeOwned>(&self, buf: &[u8]) -> Result<T, GatewayError>;
}

/// Implements `Codec` for the Protocol Buffers binary format.
///
/// This codec handles the `application/octet-stream` content type.
#[derive(Debug, Clone, Copy)]
pub struct ProtobufCodec;

impl Codec for ProtobufCodec {
    const CONTENT_TYPE: &'static str = "application/octet-stream";

    /// Encodes a message using `prost`.
    ///
    /// # Parameters
    /// *   `val`: The message to encode.
    ///
    /// # Returns
    /// The binary protobuf encoding of the message.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if the encoding process fails.
    fn encode<T: Message + serde::Serialize>(&self, val: &T) -> Result<Bytes, GatewayError> {
        let mut buf = Vec::new();
        val.encode(&mut buf)
            .map_err(|e| GatewayError::Encoding(
                #[cfg(feature = "std")]
                Box::new(e),
                #[cfg(not(feature = "std"))]
                e.to_string(),
            ))?;
        Ok(Bytes::from(buf))
    }

    /// Decodes a message using `prost`.
    ///
    /// # Parameters
    /// *   `buf`: The binary data to decode.
    ///
    /// # Returns
    /// The decoded message object.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if the data cannot be decoded into the target type.
    fn decode<T: Message + Default + DeserializeOwned>(&self, buf: &[u8]) -> Result<T, GatewayError> {
        T::decode(buf).map_err(|e| GatewayError::Encoding(
            #[cfg(feature = "std")]
            Box::new(e),
            #[cfg(not(feature = "std"))]
            e.to_string(),
        ))
    }
}

/// Implements `Codec` for the JSON format.
///
/// This codec handles the `application/json` content type.
#[derive(Debug, Clone, Copy)]
pub struct JsonCodec;

impl Codec for JsonCodec {
    const CONTENT_TYPE: &'static str = "application/json";

    /// Encodes a message using `serde_json`.
    ///
    /// # Parameters
    /// *   `val`: The message to encode.
    ///
    /// # Returns
    /// The JSON string representation of the message as bytes.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if serialization fails.
    fn encode<T: Message + serde::Serialize>(&self, val: &T) -> Result<Bytes, GatewayError> {
        let vec = serde_json::to_vec(val).map_err(|e| GatewayError::Encoding(
            #[cfg(feature = "std")]
            Box::new(e),
            #[cfg(not(feature = "std"))]
            e.to_string(),
        ))?;
        Ok(Bytes::from(vec))
    }

    /// Decodes a message using `serde_json`.
    ///
    /// # Parameters
    /// *   `buf`: The JSON data to decode.
    ///
    /// # Returns
    /// The decoded message object.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if the JSON is invalid or cannot map to the target type.
    fn decode<T: Message + Default + DeserializeOwned>(&self, buf: &[u8]) -> Result<T, GatewayError> {
        serde_json::from_slice(buf).map_err(|e| GatewayError::Encoding(
            #[cfg(feature = "std")]
            Box::new(e),
            #[cfg(not(feature = "std"))]
            e.to_string(),
        ))
    }
}
