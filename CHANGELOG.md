# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2025-02-13

### Added
- **Runtime:** Introduced a robust **Governance Layer** with configurable global concurrency limits, request timeouts, rate limiting (token bucket), and automatic load shedding.
- **Runtime:** Added **Health Check Module** (`HealthService`) supporting standard gRPC health checks (`grpc.health.v1`) and HTTP Liveness/Readiness probes (`/healthz`, `/readyz`).
- **Runtime:** Added `BodyLimitLayer` to enforce strict size limits on request and response bodies.
- **Runtime:** Added `GatewayRetryPolicy` to handle transient failures (e.g., 503, upstream errors) with configurable retry attempts.
- **Runtime:** Integrated `tower` and `tower-http` ecosystem for robust middleware support (Compression, CORS, Tracing).
- **Runtime:** Added adapters (`VecBody`, `VecBodyToVecService`) to bridge the internal `Vec<u8>` request model with standard `http_body::Body` middleware.
- **Codegen:** Added support for `extern_path` parameter (e.g., `extern_path=.google.protobuf=::pbjson_types`), allowing substitution of Protobuf types with custom Rust types or external crate types.

### Changed
- **Breaking:** Refactored `gateway-runtime` module structure. Middleware layers and types are now organized under the `layers` module (`layers::governance`, `layers::health`, `layers::tracing`, etc.).
- **Breaking:** `Gateway::into_service` now constructs a fully governed `tower::Service` stack, including buffering and error mapping.
- **Breaking:** Deprecated `with_metrics_recorder` and `with_tracing` in favor of standard `tower-http` TraceLayer integration.
- **Documentation:** Comprehensive cleanup of documentation to be professional and suitable for public release.

## [0.2.1] - 2025-02-12

### Changed
- **Codegen:** Refactored to use `protoc-gen-prost` infrastructure for robust code generation.
- **Codegen:** Enabled `source_relative` mode by default (`source_relative=true`).
- **Codegen:** Changed generated code structure: now generates `pub mod {service_name}_gw` per service instead of a single package-level module.
- **Codegen:** Removed `use_tonic_client` option (behavior is now default) and `insert_include` option (replaced by `no_include`).
- **Codegen:** `no_include` option added (defaults to false) to disable automatic generation of `include!` macros in `mod.rs`.
- **Codegen:** Generated client references now rely on relative imports, requiring generated files to be `include!`-ed alongside `tonic` generated code for correct `super` resolution.

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
