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
//! -   `MultimediaCodec`: A codec that selects between JSON and Protocol Buffers based on MIME types.
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
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use bytes::Bytes;
use prost::Message;
use serde::de::DeserializeOwned;

/// Defines how to encode and decode gRPC messages to/from HTTP bodies.
///
/// This trait abstracts the serialization logic, enabling the gateway to support various
/// wire formats.
pub trait Codec: Send + Sync + 'static {
    /// Returns the content type that this codec will use for encoding.
    ///
    /// # Parameters
    /// * `accept`: The `Accept` header value from the request, if any.
    fn encoder_content_type(&self, accept: Option<&str>) -> String;

    /// Encodes a message into a buffer.
    ///
    /// # Parameters
    /// *   `val`: The message to encode. Must implement `prost::Message` and `serde::Serialize`.
    /// *   `mime`: The MIME type requested (e.g. from Accept header).
    ///
    /// # Returns
    /// A `Result` containing the encoded bytes as `bytes::Bytes` or a `GatewayError` on failure.
    fn encode<T: Message + serde::Serialize>(
        &self,
        val: &T,
        mime: Option<&str>,
    ) -> Result<Bytes, GatewayError>;

    /// Decodes a buffer into a message.
    ///
    /// # Parameters
    /// *   `buf`: The byte slice to decode.
    /// *   `mime`: The content type of the incoming data.
    ///
    /// # Returns
    /// A `Result` containing the decoded message of type `T` or a `GatewayError` on failure.
    fn decode<T: Message + Default + DeserializeOwned>(
        &self,
        buf: &[u8],
        mime: Option<&str>,
    ) -> Result<T, GatewayError>;
}

/// Implements `Codec` for the Protocol Buffers binary format.
///
/// This codec handles the `application/octet-stream` content type.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProtobufCodec;

impl Codec for ProtobufCodec {
    fn encoder_content_type(&self, _accept: Option<&str>) -> String {
        "application/octet-stream".to_string()
    }

    /// Encodes a message using `prost`.
    ///
    /// # Parameters
    /// *   `val`: The message to encode.
    /// *   `_mime`: Ignored.
    ///
    /// # Returns
    /// The binary protobuf encoding of the message.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if the encoding process fails.
    fn encode<T: Message + serde::Serialize>(
        &self,
        val: &T,
        _mime: Option<&str>,
    ) -> Result<Bytes, GatewayError> {
        let mut buf = Vec::new();
        val.encode(&mut buf).map_err(|e| {
            GatewayError::Encoding(
                #[cfg(feature = "std")]
                Box::new(e),
                #[cfg(not(feature = "std"))]
                e.to_string(),
            )
        })?;
        Ok(Bytes::from(buf))
    }

    /// Decodes a message using `prost`.
    ///
    /// # Parameters
    /// *   `buf`: The binary data to decode.
    /// *   `_mime`: Ignored.
    ///
    /// # Returns
    /// The decoded message object.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if the data cannot be decoded into the target type.
    fn decode<T: Message + Default + DeserializeOwned>(
        &self,
        buf: &[u8],
        _mime: Option<&str>,
    ) -> Result<T, GatewayError> {
        T::decode(buf).map_err(|e| {
            GatewayError::Encoding(
                #[cfg(feature = "std")]
                Box::new(e),
                #[cfg(not(feature = "std"))]
                e.to_string(),
            )
        })
    }
}

/// Options for JSON encoding.
#[derive(Debug, Clone, Default)]
pub struct JsonEncoderOptions {
    /// Whether to format the output in indented-form with every textual element on a new line.
    /// If `indent` is empty, then an arbitrary indent (usually 2 spaces) is chosen if this is true.
    pub pretty_print: bool,

    /// The set of indentation characters to use in a multiline formatted output.
    /// If non-empty, then `pretty_print` is treated as true.
    pub indent: String,
}

/// Options for JSON decoding.
#[derive(Debug, Clone, Default)]
pub struct JsonDecoderOptions {
    // Reserved for future options like recursion limit.
}

/// Implements `Codec` for the JSON format.
///
/// This codec handles the `application/json` content type.
#[derive(Debug, Clone, Default)]
pub struct JsonCodec {
    encoder_opts: JsonEncoderOptions,
    decoder_opts: JsonDecoderOptions,
}

impl JsonCodec {
    /// Creates a new `JsonCodec` with default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `JsonCodec` with pretty printing enabled (default indentation).
    pub fn pretty() -> Self {
        Self {
            encoder_opts: JsonEncoderOptions {
                pretty_print: true,
                indent: String::new(),
            },
            decoder_opts: JsonDecoderOptions::default(),
        }
    }

    /// Configures the codec with the given encoder options.
    pub fn with_encoder_options(mut self, options: JsonEncoderOptions) -> Self {
        self.encoder_opts = options;
        self
    }

    /// Configures the codec with the given decoder options.
    pub fn with_decoder_options(mut self, options: JsonDecoderOptions) -> Self {
        self.decoder_opts = options;
        self
    }
}

impl Codec for JsonCodec {
    fn encoder_content_type(&self, _accept: Option<&str>) -> String {
        "application/json".to_string()
    }

    /// Encodes a message using `serde_json`.
    ///
    /// # Parameters
    /// *   `val`: The message to encode.
    /// *   `_mime`: Ignored.
    ///
    /// # Returns
    /// The JSON string representation of the message as bytes.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if serialization fails.
    fn encode<T: Message + serde::Serialize>(
        &self,
        val: &T,
        _mime: Option<&str>,
    ) -> Result<Bytes, GatewayError> {
        let mut buf = Vec::new();
        let indent_str = &self.encoder_opts.indent;
        let pretty_print = self.encoder_opts.pretty_print;

        let res = if pretty_print || !indent_str.is_empty() {
            let indent = if indent_str.is_empty() {
                b"  "
            } else {
                indent_str.as_bytes()
            };
            let formatter = serde_json::ser::PrettyFormatter::with_indent(indent);
            let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
            val.serialize(&mut ser)
        } else {
            serde_json::to_writer(&mut buf, val)
        };

        res.map_err(|e| {
            GatewayError::Encoding(
                #[cfg(feature = "std")]
                Box::new(e),
                #[cfg(not(feature = "std"))]
                e.to_string(),
            )
        })?;
        Ok(Bytes::from(buf))
    }

    /// Decodes a message using `serde_json`.
    ///
    /// # Parameters
    /// *   `buf`: The JSON data to decode.
    /// *   `_mime`: Ignored.
    ///
    /// # Returns
    /// The decoded message object.
    ///
    /// # Errors
    /// Returns `GatewayError::Encoding` if the JSON is invalid or cannot map to the target type.
    fn decode<T: Message + Default + DeserializeOwned>(
        &self,
        buf: &[u8],
        _mime: Option<&str>,
    ) -> Result<T, GatewayError> {
        serde_json::from_slice(buf).map_err(|e| {
            GatewayError::Encoding(
                #[cfg(feature = "std")]
                Box::new(e),
                #[cfg(not(feature = "std"))]
                e.to_string(),
            )
        })
    }
}

/// Implements `Codec` for both JSON and Protocol Buffers formats.
///
/// This codec selects the appropriate format based on the `Content-Type` (for decoding)
/// and `Accept` (for encoding) headers.
#[derive(Debug, Clone, Default)]
pub struct MultimediaCodec {
    json: JsonCodec,
    proto: ProtobufCodec,
}

impl MultimediaCodec {
    /// Creates a new `MultimediaCodec` with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `MultimediaCodec` with the provided codec instances.
    pub fn with_codecs(json: JsonCodec, proto: ProtobufCodec) -> Self {
        Self { json, proto }
    }
}

impl Codec for MultimediaCodec {
    fn encoder_content_type(&self, accept: Option<&str>) -> String {
        if let Some(accept) = accept {
            if accept.contains("application/octet-stream")
                || accept.contains("application/x-protobuf")
            {
                return "application/octet-stream".to_string();
            }
        }
        // Default to JSON
        "application/json".to_string()
    }

    fn encode<T: Message + serde::Serialize>(
        &self,
        val: &T,
        mime: Option<&str>,
    ) -> Result<Bytes, GatewayError> {
        let content_type = self.encoder_content_type(mime);
        if content_type == "application/octet-stream" {
            self.proto.encode(val, mime)
        } else {
            self.json.encode(val, mime)
        }
    }

    fn decode<T: Message + Default + DeserializeOwned>(
        &self,
        buf: &[u8],
        mime: Option<&str>,
    ) -> Result<T, GatewayError> {
        if let Some(mime) = mime {
            if mime.contains("application/octet-stream") || mime.contains("application/x-protobuf")
            {
                return self.proto.decode(buf, Some(mime));
            }
        }
        self.json.decode(buf, mime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, PartialEq, Message, Serialize, Deserialize)]
    struct TestMessage {
        #[prost(string, tag = "1")]
        pub name: String,
        #[prost(int32, tag = "2")]
        pub id: i32,
    }

    #[test]
    fn test_protobuf_codec_encoding() {
        let codec = ProtobufCodec;
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        let encoded = codec.encode(&msg, None).unwrap();

        let mut expected = Vec::new();
        msg.encode(&mut expected).unwrap();

        assert_eq!(encoded, Bytes::from(expected));
        assert_eq!(codec.encoder_content_type(None), "application/octet-stream");
    }

    #[test]
    fn test_protobuf_codec_decoding() {
        let codec = ProtobufCodec;
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        let mut buf = Vec::new();
        msg.encode(&mut buf).unwrap();

        let decoded: TestMessage = codec.decode(&buf, None).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_json_codec_default_encoding() {
        let codec = JsonCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        let encoded = codec.encode(&msg, None).unwrap();

        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(json_str, r#"{"name":"test","id":123}"#);
        assert_eq!(codec.encoder_content_type(None), "application/json");
    }

    #[test]
    fn test_json_codec_pretty_encoding() {
        let codec = JsonCodec::pretty();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        let encoded = codec.encode(&msg, None).unwrap();

        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        // Check for newlines and indentation
        assert!(json_str.contains('\n'));
        assert!(json_str.contains("  \"name\""));
    }

    #[test]
    fn test_json_codec_custom_indent_encoding() {
        let options = JsonEncoderOptions {
            pretty_print: true,
            indent: "\t".to_string(),
        };
        let codec = JsonCodec::new().with_encoder_options(options);
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        let encoded = codec.encode(&msg, None).unwrap();

        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        assert!(json_str.contains("\t\"name\""));
    }

    #[test]
    fn test_json_codec_decoding() {
        let codec = JsonCodec::new();
        let json_data = r#"{"name":"test","id":123}"#;
        let decoded: TestMessage = codec.decode(json_data.as_bytes(), None).unwrap();

        assert_eq!(decoded.name, "test");
        assert_eq!(decoded.id, 123);
    }

    #[test]
    fn test_multimedia_codec_default_encoding() {
        let codec = MultimediaCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };

        // No Accept header -> defaults to JSON
        let encoded = codec.encode(&msg, None).unwrap();
        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(json_str, r#"{"name":"test","id":123}"#);
        assert_eq!(codec.encoder_content_type(None), "application/json");
    }

    #[test]
    fn test_multimedia_codec_json_encoding() {
        let codec = MultimediaCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };

        let encoded = codec.encode(&msg, Some("application/json")).unwrap();
        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(json_str, r#"{"name":"test","id":123}"#);
        assert_eq!(
            codec.encoder_content_type(Some("application/json")),
            "application/json"
        );
    }

    #[test]
    fn test_multimedia_codec_protobuf_encoding() {
        let codec = MultimediaCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };

        let encoded = codec
            .encode(&msg, Some("application/octet-stream"))
            .unwrap();
        let mut expected = Vec::new();
        msg.encode(&mut expected).unwrap();
        assert_eq!(encoded, Bytes::from(expected));
        assert_eq!(
            codec.encoder_content_type(Some("application/octet-stream")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_multimedia_codec_protobuf_alias_encoding() {
        let codec = MultimediaCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };

        // Check x-protobuf alias
        let encoded = codec.encode(&msg, Some("application/x-protobuf")).unwrap();
        let mut expected = Vec::new();
        msg.encode(&mut expected).unwrap();
        assert_eq!(encoded, Bytes::from(expected));
        assert_eq!(
            codec.encoder_content_type(Some("application/x-protobuf")),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_multimedia_codec_wildcard_json_decoding() {
        let codec = MultimediaCodec::new();
        let json_data = r#"{"name":"test","id":123}"#;

        // No Content-Type -> defaults to JSON
        let decoded: TestMessage = codec.decode(json_data.as_bytes(), None).unwrap();
        assert_eq!(decoded.name, "test");
        assert_eq!(decoded.id, 123);
    }

    #[test]
    fn test_multimedia_codec_explicit_json_decoding() {
        let codec = MultimediaCodec::new();
        let json_data = r#"{"name":"test","id":123}"#;

        let decoded: TestMessage = codec
            .decode(json_data.as_bytes(), Some("application/json"))
            .unwrap();
        assert_eq!(decoded.name, "test");
        assert_eq!(decoded.id, 123);
    }

    #[test]
    fn test_multimedia_codec_protobuf_decoding() {
        let codec = MultimediaCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        let mut buf = Vec::new();
        msg.encode(&mut buf).unwrap();

        let decoded: TestMessage = codec
            .decode(&buf, Some("application/octet-stream"))
            .unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn test_multimedia_codec_with_custom_codecs() {
        let json_codec = JsonCodec::pretty();
        let proto_codec = ProtobufCodec;
        let codec = MultimediaCodec::with_codecs(json_codec, proto_codec);

        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };

        // Should use pretty JSON
        let encoded = codec.encode(&msg, None).unwrap();
        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        assert!(json_str.contains('\n'));
    }

    #[test]
    fn test_json_decoding_error() {
        let codec = JsonCodec::new();
        let bad_json = r#"{"name": 123}"#; // name expects string, got int
        let result: Result<TestMessage, _> = codec.decode(bad_json.as_bytes(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_protobuf_decoding_error() {
        let codec = ProtobufCodec;
        let bad_proto = vec![255, 255, 255]; // Invalid varint or similar garbage
        let result: Result<TestMessage, _> = codec.decode(&bad_proto, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_multimedia_fallback_decoding() {
        let codec = MultimediaCodec::new();
        // Unknown content type -> defaults to JSON
        let json_data = r#"{"name":"test","id":123}"#;
        let decoded: TestMessage = codec
            .decode(json_data.as_bytes(), Some("text/plain"))
            .unwrap();
        assert_eq!(decoded.name, "test");
    }

    #[test]
    fn test_multimedia_fallback_encoding() {
        let codec = MultimediaCodec::new();
        let msg = TestMessage {
            name: "test".to_string(),
            id: 123,
        };
        // Unknown accept type -> defaults to JSON
        let encoded = codec.encode(&msg, Some("text/html")).unwrap();
        let json_str = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(json_str, r#"{"name":"test","id":123}"#);
    }

    #[test]
    fn test_json_codec_empty_input() {
        let codec = JsonCodec::new();
        let result: Result<TestMessage, _> = codec.decode(&[], None);
        assert!(result.is_err()); // Empty JSON is invalid for a struct
    }

    #[test]
    fn test_protobuf_codec_empty_input() {
        let codec = ProtobufCodec;
        let result: Result<TestMessage, _> = codec.decode(&[], None);
        // Empty bytes are valid for protobuf (defaults everything)
        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded.name, "");
        assert_eq!(decoded.id, 0);
    }
}
