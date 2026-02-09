# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-02-09

### Added
- **Runtime:** Completely refactored middleware architecture using `tower` layers.
- **Runtime:** `Gateway` builder pattern for secure and flexible configuration.
- **Runtime:** Comprehensive default handlers mimicking `grpc-gateway` behavior:
    - **Error Handling:** JSON error responses for upstream gRPC errors.
    - **Metadata:** Automatic generation of secure `x-request-id` and Client IP injection.
    - **Headers:** Security filtering (blocking `x-request-id`, `Authorization`) and standard renaming rules.
    - **Responses:** Support for mapping `x-http-code` metadata to HTTP status codes.
- **Runtime:** Support for API Key verification via `RouteMetadata` and `AuthVerifier`.
- **Runtime:** `UnescapingMode` configuration for handling percent-encoded paths.
- **Runtime:** Graceful shutdown utility (`ShutdownSignal`).
- **Runtime:** Tracing and Metrics hooks.

### Changed
- **Breaking:** `Gateway::handle` replaced with `Gateway::into_service()`, which returns a `tower::BoxCloneService`.
- **Breaking:** `Router::match_request` signature updated to return `RouteMetadata` alongside service and params.
- **Security:** Strict filtering of incoming headers to prevent context spoofing (e.g., stripping external `x-request-id`).

## [0.1.1] - 2025-02-08

### Added
- **Runtime:** `JsonCodec` now supports pretty-printing and custom indentation via `JsonEncoderOptions`.
- **Runtime:** Introduced `MultimediaCodec` which acts as a registry/dispatcher, selecting between `JsonCodec` and `ProtobufCodec` based on `Accept` (for encoding) and `Content-Type` (for decoding) headers.
- **Runtime:** `Codec` trait now supports content negotiation by accepting MIME type arguments in `encode`/`decode` methods.

### Changed
- **Breaking:** Changed generated file extension from `.rs` to `.gw.rs` to avoid conflicts with other plugins (e.g., `tonic-build`).
- **Breaking:** Changed output filename generation to use the Protobuf `package` directive (e.g., `my.package.v1.gw.rs`) instead of mirroring the input file path. If no package is defined, it falls back to `{filename}.gw.rs`. This flattens the output directory structure.
- **Breaking:** `JsonCodec` is no longer a unit struct; use `JsonCodec::new()` or `JsonCodec::pretty()` to instantiate.
- **Breaking:** `Codec` trait signature updated to include `mime` parameter in `encode` and `decode` methods.

## [0.1.0] - 2025-02-08

### Added
- Initial release of `grpc-gateway-rust`.
