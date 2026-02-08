# protoc-gen-grpc-gateway

This is the `protoc` plugin for the Rust gRPC Gateway. It generates the necessary Rust code to run a reverse proxy server that translates RESTful HTTP API calls into gRPC.

## Installation

You can install the plugin directly from crates.io using `cargo install`:

```bash
cargo install protoc-gen-grpc-gateway
```

This will install the `protoc-gen-grpc-gateway` binary into your `~/.cargo/bin` directory. Ensure this directory is in your system's `PATH`.

## Usage with `protoc`

Once installed, you can use the plugin with `protoc` by specifying the `--grpc-gateway-rust_out` option.

### Basic Example

Assuming you have a `service.proto` file and `protoc` installed:

```bash
protoc \
    --proto_path=. \
    --experimental_allow_proto3_optional \
    --grpc-gateway-rust_out=./generated \
    service.proto
```

This command will:

1. Read `service.proto`.
2. Invoke `protoc-gen-grpc-gateway`.
3. Output the generated Rust code into the `./generated` directory. The output file will be named `{package}.gw.rs` (e.g., `example.service.v1.gw.rs` if the package is
   `example.service.v1`). If no package is defined, it defaults to `{filename}.gw.rs`.

### Advanced Options

The plugin supports several options that can be passed via the `--grpc-gateway-rust_opt` flag (or appended to `_out` separated by colons).

| Option        | Description                                                                                       | Default |
|---------------|---------------------------------------------------------------------------------------------------|---------|
| `log_level`   | Set the logging level (`error`, `warn`, `info`, `debug`, `trace`).                                | `info`  |
| `module_path` | Prefix for module paths if your proto package structure doesn't match your Rust module structure. | (empty) |

Example with options:

```bash
protoc \
    --proto_path=. \
    --grpc-gateway-rust_out=log_level=debug,module_path=crate::pb:./generated \
    service.proto
```

## Integration with `tonic-build`

If you are using `tonic-build` (or `tonic-prost-build`) in a `build.rs` script, you generally don't need to run `protoc` manually. However, you need to configure the build script
to use this plugin.

See the [main README](../README.md#getting-started) for detailed instructions on `build.rs` integration.
