# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-02-08

### Added
- Initial release of `grpc-gateway-rust`.


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
