use gateway_annotations::google::api::{http_rule::Pattern, HttpRule};
use gateway_annotations::google::protobuf_custom::{
    source_code_info::Location, DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    MethodDescriptorProto, MethodOptions, ServiceDescriptorProto, SourceCodeInfo,
};
use gateway_codegen::{descriptor_processor, generator};

#[test]
fn test_documentation_generation() {
    // Construct FileDescriptorProto
    let mut location = Location::default();
    location.path = vec![6, 0];
    location.leading_comments =
        Some("Service documentation line 1.\nService documentation line 2.".to_string());

    let mut method_location = Location::default();
    method_location.path = vec![6, 0, 2, 0];
    method_location.leading_comments = Some("Method documentation.".to_string());

    let mut req_msg_loc = Location::default();
    req_msg_loc.path = vec![4, 0];
    req_msg_loc.leading_comments = Some("Request doc".to_string());

    let mut req_field_loc = Location::default();
    req_field_loc.path = vec![4, 0, 2, 0];
    req_field_loc.leading_comments = Some("Field doc".to_string());

    let mut resp_msg_loc = Location::default();
    resp_msg_loc.path = vec![4, 1];
    resp_msg_loc.leading_comments = Some("Response doc".to_string());

    let source_info = SourceCodeInfo {
        location: vec![
            location,
            method_location,
            req_msg_loc,
            req_field_loc,
            resp_msg_loc,
        ],
    };

    let method = MethodDescriptorProto {
        name: Some("Echo".to_string()),
        input_type: Some(".test.EchoRequest".to_string()),
        output_type: Some(".test.EchoResponse".to_string()),
        options: Some(MethodOptions {
            http: Some(HttpRule {
                pattern: Some(Pattern::Get("/v1/echo".to_string())),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service = ServiceDescriptorProto {
        name: Some("TestService".to_string()),
        method: vec![method],
        ..Default::default()
    };

    let echo_request = DescriptorProto {
        name: Some("EchoRequest".to_string()),
        field: vec![FieldDescriptorProto {
            name: Some("data".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let echo_response = DescriptorProto {
        name: Some("EchoResponse".to_string()),
        ..Default::default()
    };

    let file = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        service: vec![service],
        message_type: vec![echo_request, echo_response],
        source_code_info: Some(source_info),
        ..Default::default()
    };

    let registry = descriptor_processor::SymbolRegistry::new(&[file.clone()]);

    // Process file
    let services = descriptor_processor::process_file(&file, &registry)
        .expect("Should succeed with valid documentation");
    assert_eq!(services.len(), 1);
    let svc_def = &services[0];

    // Check service docs in definition
    assert_eq!(svc_def.docs.len(), 2);
    assert_eq!(svc_def.docs[0], "Service documentation line 1.");
    assert_eq!(svc_def.docs[1], "Service documentation line 2.");
    assert_eq!(svc_def.proto_file, "test.proto");

    // Check method docs in definition
    assert_eq!(svc_def.methods.len(), 1);
    let method_def = &svc_def.methods[0];
    assert_eq!(method_def.docs.len(), 1);
    assert_eq!(method_def.docs[0], "Method documentation.");
    assert_eq!(method_def.proto_service, "TestService");
    assert_eq!(method_def.proto_file, "test.proto");

    // Generate code
    let tokens = generator::generate_service(svc_def);
    let output = tokens.to_string();

    assert!(output.contains("# [doc = \"Service documentation line 1.\"]"));
    assert!(output.contains("# [doc = \"Service documentation line 2.\"]"));

    // Service footer is removed.

    assert!(output.contains("# [doc = \"Method documentation.\"]"));

    // Method footer is suppressed if docs are present.
}

#[test]
fn test_missing_documentation_fallback() {
    let method = MethodDescriptorProto {
        name: Some("Echo".to_string()),
        input_type: Some(".test.EchoRequest".to_string()),
        output_type: Some(".test.EchoResponse".to_string()),
        options: Some(MethodOptions {
            http: Some(HttpRule {
                pattern: Some(Pattern::Get("/v1/echo".to_string())),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service = ServiceDescriptorProto {
        name: Some("TestService".to_string()),
        method: vec![method],
        ..Default::default()
    };

    let file = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        service: vec![service],
        source_code_info: Some(SourceCodeInfo { location: vec![] }),
        ..Default::default()
    };

    let registry = descriptor_processor::SymbolRegistry::new(&[file.clone()]);
    let services = descriptor_processor::process_file(&file, &registry)
        .expect("Should succeed with fallback documentation");
    assert_eq!(services.len(), 1);
    let svc_def = &services[0];

    // Check fallback service docs: Should be empty
    assert!(svc_def.docs.is_empty());

    // Check fallback method docs: Should be empty
    assert_eq!(svc_def.methods.len(), 1);
    let method_def = &svc_def.methods[0];
    assert!(method_def.docs.is_empty());

    // Generate code and check for footer
    let tokens = generator::generate_service(svc_def);
    let output = tokens.to_string();

    // Method footer should be present
    let expected_method_footer =
        "This endpoint is generated from TestService.Echo defined in test.proto.";
    assert!(output.contains(expected_method_footer));
}

#[test]
fn test_undocumented_field_success() {
    let mut location = Location::default();
    location.path = vec![6, 0];
    location.leading_comments = Some("Svc doc".to_string());

    let mut method_location = Location::default();
    method_location.path = vec![6, 0, 2, 0];
    method_location.leading_comments = Some("Method doc".to_string());

    let mut req_msg_loc = Location::default();
    req_msg_loc.path = vec![4, 0];
    req_msg_loc.leading_comments = Some("Message doc".to_string());

    let source_info = SourceCodeInfo {
        location: vec![location, method_location, req_msg_loc],
    };

    let method = MethodDescriptorProto {
        name: Some("Echo".to_string()),
        input_type: Some(".test.EchoRequest".to_string()),
        output_type: Some(".test.EchoResponse".to_string()),
        options: Some(MethodOptions {
            http: Some(HttpRule {
                pattern: Some(Pattern::Get("/v1/echo".to_string())),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service = ServiceDescriptorProto {
        name: Some("TestService".to_string()),
        method: vec![method],
        ..Default::default()
    };

    let echo_request = DescriptorProto {
        name: Some("EchoRequest".to_string()),
        field: vec![FieldDescriptorProto {
            name: Some("data".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let echo_response = DescriptorProto {
        name: Some("EchoResponse".to_string()),
        ..Default::default()
    };

    let file = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        service: vec![service],
        message_type: vec![echo_request, echo_response],
        source_code_info: Some(source_info),
        ..Default::default()
    };

    let registry = descriptor_processor::SymbolRegistry::new(&[file.clone()]);
    let result = descriptor_processor::process_file(&file, &registry);
    assert!(result.is_ok());
}

#[test]
fn test_undocumented_message_success() {
    let mut location = Location::default();
    location.path = vec![6, 0];
    location.leading_comments = Some("Svc doc".to_string());

    let mut method_location = Location::default();
    method_location.path = vec![6, 0, 2, 0];
    method_location.leading_comments = Some("Method doc".to_string());

    let mut req_field_loc = Location::default();
    req_field_loc.path = vec![4, 0, 2, 0];
    req_field_loc.leading_comments = Some("Field doc".to_string());

    let source_info = SourceCodeInfo {
        location: vec![location, method_location, req_field_loc],
    };

    let method = MethodDescriptorProto {
        name: Some("Echo".to_string()),
        input_type: Some(".test.EchoRequest".to_string()),
        output_type: Some(".test.EchoResponse".to_string()),
        options: Some(MethodOptions {
            http: Some(HttpRule {
                pattern: Some(Pattern::Get("/v1/echo".to_string())),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service = ServiceDescriptorProto {
        name: Some("TestService".to_string()),
        method: vec![method],
        ..Default::default()
    };

    let echo_request = DescriptorProto {
        name: Some("EchoRequest".to_string()),
        field: vec![FieldDescriptorProto {
            name: Some("data".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let echo_response = DescriptorProto {
        name: Some("EchoResponse".to_string()),
        ..Default::default()
    };

    let file = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        service: vec![service],
        message_type: vec![echo_request, echo_response],
        source_code_info: Some(source_info),
        ..Default::default()
    };

    let registry = descriptor_processor::SymbolRegistry::new(&[file.clone()]);
    let result = descriptor_processor::process_file(&file, &registry);
    assert!(result.is_ok());
}
