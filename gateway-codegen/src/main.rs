use gateway_annotations::google::protobuf_custom::compiler::CodeGeneratorRequest as CustomCodeGeneratorRequest;
use gateway_codegen::generator::GrpcGatewayGenerator;
use prost::Message;
use prost_types::compiler::{CodeGeneratorRequest, CodeGeneratorResponse};
use protoc_gen_prost::{Generator, ModuleRequestSet};
use std::collections::HashMap;
use std::io::{self, Read, Write};

fn main() -> io::Result<()> {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf)?;

    // Parse with gateway_annotations to get rich descriptors (with extensions) and parameters
    let rich_request = CustomCodeGeneratorRequest::decode(&buf[..])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut source_relative = true;
    let mut no_include = false;
    let mut extern_paths = HashMap::new();

    if let Some(params) = &rich_request.parameter {
        for param in params.split(',') {
            let p = param.trim();
            if p == "paths=source_relative" {
                source_relative = true;
            } else if p == "paths=import" {
                source_relative = false;
            } else if p == "no_include" || p == "no_include=true" {
                no_include = true;
            } else if let Some(pair) = p.strip_prefix("extern_path=") {
                if let Some((proto_path, rust_path)) = pair.split_once('=') {
                    extern_paths.insert(proto_path.to_string(), rust_path.to_string());
                }
            }
        }
    }

    let mut file_map = HashMap::new();
    for file in rich_request.proto_file {
        if let Some(name) = &file.name {
            file_map.insert(name.clone(), file);
        }
    }

    // Parse with prost_types for protoc-gen-prost compatibility (lean)
    let request = CodeGeneratorRequest::decode(&buf[..])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let module_request_set = ModuleRequestSet::new(
        request.file_to_generate,
        request.proto_file,
        &buf,
        None,
        !source_relative,
    )
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut generator = GrpcGatewayGenerator::new(file_map, extern_paths);
    generator.source_relative = source_relative;
    generator.no_include = no_include;

    let files = generator
        .generate(&module_request_set)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    let response = CodeGeneratorResponse {
        file: files,
        supported_features: Some(1), // FEATURE_PROTO3_OPTIONAL
        ..Default::default()
    };

    let mut out_buf = Vec::new();
    response
        .encode(&mut out_buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    io::stdout().write_all(&out_buf)?;
    Ok(())
}
