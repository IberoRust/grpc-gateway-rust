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

    let tokens = generator::generate_service(&service);
    let wrapped = quote! { #tokens };

    // Verify it parses as a file (valid Rust syntax)
    let _: syn::File = syn::parse2(wrapped).expect("Generated code should be valid Rust");
}
