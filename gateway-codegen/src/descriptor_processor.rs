use crate::path_compiler;
use gateway_annotations::get_http_rule;
use gateway_annotations::google::api::HttpRule as ProtoHttpRule;
use gateway_annotations::google::protobuf_custom::{
    DescriptorProto, FileDescriptorProto, MethodDescriptorProto, ServiceDescriptorProto,
};
use gateway_internal::body_mapping::BodyMapping;
use gateway_internal::method_binding::{MethodBinding, Parameter};
use gateway_internal::path_template::PathTemplate;
use std::collections::HashMap;

pub struct ServiceDefinition {
    pub name: String,
    pub package: String,
    pub docs: Vec<String>,
    pub proto_file: String,
    pub methods: Vec<MethodDefinition>,
}

pub struct MethodDefinition {
    pub name: String,
    pub input_type: String,
    pub output_type: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub docs: Vec<String>,
    pub proto_service: String,
    pub proto_file: String,
    pub bindings: Vec<MethodBinding>,
}

struct CommentMap {
    map: HashMap<Vec<i32>, String>,
}

impl CommentMap {
    fn get(&self, path: &[i32]) -> Vec<String> {
        if let Some(comment) = self.map.get(path) {
            comment.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    }
}

pub struct SymbolRegistry {
    messages: HashMap<String, MessageDocs>,
}

struct MessageDocs {
    #[allow(dead_code)]
    docs: Vec<String>,
    fields: HashMap<String, FieldDocs>,
    #[allow(dead_code)]
    file: String,
}

struct FieldDocs {
    #[allow(dead_code)]
    docs: Vec<String>,
    label: Option<i32>,
    type_name: Option<String>,
    field_type: Option<i32>,
}

impl SymbolRegistry {
    pub fn new(files: &[FileDescriptorProto]) -> Self {
        let mut registry = SymbolRegistry {
            messages: HashMap::new(),
        };
        for file in files {
            registry.process_file(file);
        }
        registry
    }

    fn process_file(&mut self, file: &FileDescriptorProto) {
        let package = file.package.clone().unwrap_or_default();
        let comment_map = process_source_info(file);
        let filename = file.name.clone().unwrap_or_default();

        let package_prefix = if package.is_empty() {
            "".to_string()
        } else {
            format!(".{}", package)
        };

        for (i, message) in file.message_type.iter().enumerate() {
            self.process_message(
                message,
                &package_prefix,
                &comment_map,
                &[4, i as i32],
                &filename,
            );
        }
    }

    fn process_message(
        &mut self,
        message: &DescriptorProto,
        parent_scope: &str,
        comments: &CommentMap,
        path: &[i32],
        filename: &str,
    ) {
        let name = message.name.clone().unwrap_or_default();
        let full_name = format!("{}.{}", parent_scope, name);

        let mut fields = HashMap::new();
        for (i, field) in message.field.iter().enumerate() {
            let mut field_path = path.to_vec();
            field_path.push(2); // field
            field_path.push(i as i32);

            let field_docs = comments.get(&field_path);
            let field_name = field.name.clone().unwrap_or_default();
            fields.insert(
                field_name,
                FieldDocs {
                    docs: field_docs,
                    label: field.label,
                    type_name: field.type_name.clone(),
                    field_type: field.r#type,
                },
            );
        }

        self.messages.insert(
            full_name.clone(),
            MessageDocs {
                docs: comments.get(path),
                fields,
                file: filename.to_string(),
            },
        );

        // Nested messages
        for (i, nested) in message.nested_type.iter().enumerate() {
            let mut nested_path = path.to_vec();
            nested_path.push(3); // nested_type
            nested_path.push(i as i32);

            self.process_message(nested, &full_name, comments, &nested_path, filename);
        }
    }

    fn resolve_field(&self, root_type: &str, field_path: &str) -> Option<&FieldDocs> {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current_type = root_type.to_string();

        for (i, part) in parts.iter().enumerate() {
            if let Some(msg) = self.messages.get(&current_type) {
                if let Some(field) = msg.fields.get(*part) {
                    if i == parts.len() - 1 {
                        return Some(field);
                    }
                    if let Some(ref type_name) = field.type_name {
                        current_type = type_name.clone();
                    } else {
                        // Cannot traverse if type_name is missing (e.g. primitive)
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        None
    }
}

fn process_source_info(file: &FileDescriptorProto) -> CommentMap {
    let mut map = HashMap::new();
    if let Some(source_info) = &file.source_code_info {
        for location in &source_info.location {
            if let Some(leading_comments) = &location.leading_comments {
                map.insert(location.path.clone(), leading_comments.clone());
            }
        }
    }
    CommentMap { map }
}

pub fn process_file(
    file: &FileDescriptorProto,
    registry: &SymbolRegistry,
) -> Result<Vec<ServiceDefinition>, String> {
    let mut services = Vec::new();
    let package = file.package.clone().unwrap_or_default();
    let comment_map = process_source_info(file);
    let filename = file.name.clone().unwrap_or_default();

    for (i, service) in file.service.iter().enumerate() {
        services.push(process_service(
            service,
            &package,
            &comment_map,
            &[6, i as i32],
            &filename,
            registry,
        )?);
    }
    Ok(services)
}

fn process_service(
    service: &ServiceDescriptorProto,
    package: &str,
    comments: &CommentMap,
    path: &[i32],
    filename: &str,
    registry: &SymbolRegistry,
) -> Result<ServiceDefinition, String> {
    let mut methods = Vec::new();
    let service_name = service.name.clone().unwrap_or_default();
    let docs = comments.get(path);

    for (i, method) in service.method.iter().enumerate() {
        let mut method_path = path.to_vec();
        method_path.push(2);
        method_path.push(i as i32);

        if let Some(method_def_result) = process_method(
            method,
            comments,
            &method_path,
            &service_name,
            filename,
            registry,
        ) {
            methods.push(method_def_result?);
        }
    }
    Ok(ServiceDefinition {
        name: service_name,
        package: package.to_string(),
        docs,
        proto_file: filename.to_string(),
        methods,
    })
}

fn process_method(
    method: &MethodDescriptorProto,
    comments: &CommentMap,
    path: &[i32],
    service_name: &str,
    filename: &str,
    registry: &SymbolRegistry,
) -> Option<Result<MethodDefinition, String>> {
    let options = method.options.as_ref()?;
    let http_rule = get_http_rule(options)?;

    // Validate Input Type
    let input_type = method.input_type.clone().unwrap_or_default();
    // Validate Output Type
    let output_type = method.output_type.clone().unwrap_or_default();

    let bindings = extract_bindings(&http_rule, &input_type, registry);

    let docs = comments.get(path);

    Some(Ok(MethodDefinition {
        name: method.name.clone().unwrap_or_default(),
        input_type,
        output_type,
        client_streaming: method.client_streaming.unwrap_or(false),
        server_streaming: method.server_streaming.unwrap_or(false),
        docs,
        proto_service: service_name.to_string(),
        proto_file: filename.to_string(),
        bindings,
    }))
}

fn extract_bindings(
    rule: &ProtoHttpRule,
    input_type: &str,
    registry: &SymbolRegistry,
) -> Vec<MethodBinding> {
    let mut bindings = Vec::new();

    if let Some(binding) = convert_rule_to_binding(rule, 0, input_type, registry) {
        bindings.push(binding);
    }

    for (i, additional) in rule.additional_bindings.iter().enumerate() {
        if let Some(binding) = convert_rule_to_binding(additional, i + 1, input_type, registry) {
            bindings.push(binding);
        }
    }

    bindings
}

fn convert_rule_to_binding(
    rule: &ProtoHttpRule,
    index: usize,
    input_type: &str,
    registry: &SymbolRegistry,
) -> Option<MethodBinding> {
    let (method, template) = match &rule.pattern {
        Some(gateway_annotations::google::api::http_rule::Pattern::Get(path)) => ("GET", path),
        Some(gateway_annotations::google::api::http_rule::Pattern::Put(path)) => ("PUT", path),
        Some(gateway_annotations::google::api::http_rule::Pattern::Post(path)) => ("POST", path),
        Some(gateway_annotations::google::api::http_rule::Pattern::Delete(path)) => {
            ("DELETE", path)
        }
        Some(gateway_annotations::google::api::http_rule::Pattern::Patch(path)) => ("PATCH", path),
        Some(gateway_annotations::google::api::http_rule::Pattern::Custom(custom)) => {
            (custom.kind.as_str(), &custom.path)
        }
        None => return None,
    };

    let compiled_pattern = path_compiler::compile(template);
    let path_params = compiled_pattern
        .vars
        .into_iter()
        .map(|v| {
            let (is_repeated, field_type) =
                if let Some(field) = registry.resolve_field(input_type, &v) {
                    (field.label == Some(3), field.field_type)
                } else {
                    // Attempt with leading dot if missing, just in case
                    if !input_type.starts_with('.') {
                        let input_type_dot = format!(".{}", input_type);
                        if let Some(field) = registry.resolve_field(&input_type_dot, &v) {
                            (field.label == Some(3), field.field_type)
                        } else {
                            (false, None)
                        }
                    } else {
                        (false, None)
                    }
                };

            Parameter {
                field_path: v.clone(), // Explicitly clone string to satisfy mismatched types
                target_type: "TYPE_STRING".to_string(),
                is_repeated,
                field_type,
            }
        })
        .collect();

    let path_tmpl = PathTemplate {
        segments: vec![], // Not needed for runtime logic, mainly for structure
        verb: compiled_pattern.verb,
        template: template.clone(),
    };

    let body = if rule.body.is_empty() {
        None
    } else {
        Some(BodyMapping {
            field_path: Some(rule.body.clone()),
        })
    };

    let response_body = if rule.response_body.is_empty() {
        None
    } else {
        Some(BodyMapping {
            field_path: Some(rule.response_body.clone()),
        })
    };

    Some(MethodBinding {
        http_method: method.to_string(),
        path_tmpl,
        index,
        path_params,
        body,
        response_body,
        query_param_filter: None,
    })
}
