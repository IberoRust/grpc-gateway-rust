use protoc_bin_vendored;

fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().unwrap();
    std::env::set_var("PROTOC", protoc);

    prost_build::Config::new()
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &[
                "proto/google/protobuf/compiler/plugin.proto",
                "proto/google/api/annotations.proto",
                "proto/google/api/http.proto",
            ],
            &["proto"],
        )
        .unwrap();
}
