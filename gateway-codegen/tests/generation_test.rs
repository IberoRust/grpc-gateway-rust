use gateway_codegen::descriptor_processor::{MethodDefinition, ServiceDefinition};
use gateway_codegen::generator;
use quote::quote;

#[test]
fn test_generation_output() {
    let service = ServiceDefinition {
        name: "TestService".to_string(),
        package: "test".to_string(),
        docs: vec!["Service documentation".to_string()],
        proto_file: "test.proto".to_string(),
        methods: vec![MethodDefinition {
            name: "Echo".to_string(),
            input_type: ".test.EchoRequest".to_string(),
            output_type: ".test.EchoResponse".to_string(),
            client_streaming: false,
            server_streaming: false,
            docs: vec!["Method documentation".to_string()],
            proto_service: "TestService".to_string(),
            proto_file: "test.proto".to_string(),
            bindings: vec![],
        }],
    };

    let options = generator::GeneratorOptions {
        source_relative: false,
    };
    let tokens = generator::generate_service(&service, &options);
    let wrapped = quote! { #tokens };

    // Verify it parses as a file (valid Rust syntax)
    let _: syn::File = syn::parse2(wrapped).expect("Generated code should be valid Rust");
}

#[test]
fn test_generation_source_relative() {
    let service = ServiceDefinition {
        name: "TestService".to_string(),
        package: "a.b.c".to_string(),
        docs: vec![],
        proto_file: "a/b/c/test.proto".to_string(),
        methods: vec![MethodDefinition {
            name: "Echo".to_string(),
            input_type: ".a.b.c.EchoRequest".to_string(),
            output_type: ".a.b.c.EchoResponse".to_string(),
            client_streaming: false,
            server_streaming: false,
            docs: vec![],
            proto_service: "TestService".to_string(),
            proto_file: "a/b/c/test.proto".to_string(),
            bindings: vec![],
        }],
    };

    let options = generator::GeneratorOptions {
        source_relative: true,
    };
    let tokens = generator::generate_service(&service, &options);
    let output = tokens.to_string();

    // Check for super::EchoRequest
    // a.b.c.EchoRequest -> super::EchoRequest (because common prefix is 3, parts 3, super_count = 1)
    // Now that we always wrap in a module, it should be super::EchoRequest
    assert!(output.contains("super :: EchoRequest"));

    // Check for client: super::test_service_client::TestServiceClient
    assert!(output.contains("super :: test_service_client :: TestServiceClient"));
}
