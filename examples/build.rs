use std::{env, path::PathBuf};

fn main() {
    // ---------- Paths ----------
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // ---------- Vendored protoc (REPRODUCIBLE) ----------
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("failed to locate vendored protoc");
    env::set_var("PROTOC", &protoc);

    // ---------- grpc-gateway ----------
    let gateway_out = out_dir.join("gateway");
    std::fs::create_dir_all(&gateway_out).unwrap();

    let plugin = format!("{}/../target/debug/protoc-gen-grpc-gateway", root.display());

    // ---------- gRPC (tonic) ----------
    let configure = tonic_prost_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .message_attribute(".", "#[serde(default)]")
        .compile_well_known_types(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .protoc_arg(format!("--plugin=protoc-gen-grpc-gateway-rust={}", plugin))
        .protoc_arg(format!("--grpc-gateway-rust_out={}", gateway_out.display()))
        .protoc_arg("--grpc-gateway-rust_opt=no_include=true")
        .build_server(true)
        .build_client(true);

    configure
        .clone()
        .compile_protos(
            &[
                "proto/examplemultipart/stored_file_service.proto",
                "proto/examplepb/a_bit_of_everything.proto",
            ],
            &["proto/"],
        )
        .expect("Failed to run protoc gateway");
}
