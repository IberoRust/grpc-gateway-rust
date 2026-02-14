//! # Generator
//!
//! ## Purpose
//! Contains the core logic for generating Rust source code from the processed service definitions.
//! It constructs the AST (Abstract Syntax Tree) using `quote` and emits the final output.

use crate::descriptor_processor::{self, ServiceDefinition};
use crate::path_compiler;
use gateway_internal::path_template::OpCode;
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::{Ident, Span, TokenStream};
use prost_types::compiler::code_generator_response::File;
use protoc_gen_prost::{Generator, ModuleRequestSet, Result};
use quote::{format_ident, quote};
use std::collections::HashMap;

/// Resolves a dot-separated protobuf type name to a Rust path token stream.
fn resolve_type(type_name: &str) -> TokenStream {
    let s = type_name.trim_start_matches('.').replace('.', "::");
    match syn::parse_str::<syn::Path>(&s) {
        Ok(path) => quote! { #path },
        Err(err) => {
            let msg = format!("Invalid protobuf type path `{}`: {}", s, err);
            quote! { compile_error!(#msg); }
        }
    }
}

/// Options for the generator.
pub struct GeneratorOptions {
    pub source_relative: bool,
    pub extern_paths: HashMap<String, String>,
}

/// Resolves a relative Rust path for a type based on the current package.
fn resolve_relative_type(
    type_name: &str,
    current_package: &str,
    options: &GeneratorOptions,
) -> TokenStream {
    // Check extern paths first
    if let Some(extern_path) = options.extern_paths.iter().find(|(proto_path, _)| {
        type_name == *proto_path || type_name.starts_with(&format!("{}.", proto_path))
    }) {
        let (proto_prefix, rust_prefix) = extern_path;
        // Replace the prefix
        // e.g. type_name = ".google.protobuf.Timestamp"
        // extern_path = ".google.protobuf" -> "::pbjson_types"
        // result = "::pbjson_types::Timestamp"

        // Handle exact match
        if type_name == *proto_prefix
            || type_name.trim_start_matches('.') == proto_prefix.trim_start_matches('.')
        {
            return resolve_type(rust_prefix);
        }

        // Handle prefix match
        // Clean leading dots for reliable comparison logic, though inputs usually have them.
        let clean_type = type_name.trim_start_matches('.');
        let clean_proto = proto_prefix.trim_start_matches('.');

        if clean_type.starts_with(clean_proto) {
            let suffix = &clean_type[clean_proto.len()..]; // e.g. ".Timestamp" or just "Timestamp" if separator is handled
                                                           // If suffix starts with '.', replace with '::' and append to rust_prefix
            let resolved = format!("{}{}", rust_prefix, suffix.replace('.', "::"));
            return resolve_type(&resolved);
        }
    }

    if !type_name.starts_with('.') {
        // Primitive or already resolved
        return resolve_type(type_name);
    }

    let type_path = type_name.trim_start_matches('.');
    let current_parts: Vec<&str> = current_package
        .split('.')
        .filter(|s| !s.is_empty())
        .collect();
    let type_parts: Vec<&str> = type_path.split('.').filter(|s| !s.is_empty()).collect();

    let mut common_prefix_len = 0;
    for (i, part) in current_parts.iter().enumerate() {
        if i < type_parts.len() && part == &type_parts[i] {
            common_prefix_len += 1;
        } else {
            break;
        }
    }

    // Always +1 because we are always in a submodule now
    let super_count = current_parts.len() - common_prefix_len + 1;
    let mut tokens = TokenStream::new();

    for _ in 0..super_count {
        tokens.extend(quote! { super:: });
    }

    for (i, part) in type_parts.iter().skip(common_prefix_len).enumerate() {
        let ident = safe_ident(part);
        if i > 0 {
            tokens.extend(quote! { :: });
        }
        tokens.extend(quote! { #ident });
    }

    tokens
}

/// Creates a safe Rust identifier from a string, escaping keywords.
fn safe_ident(name: &str) -> Ident {
    match name {
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern" | "false"
        | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move"
        | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static" | "struct" | "super"
        | "trait" | "true" | "type" | "unsafe" | "use" | "where" | "while" | "async" | "await"
        | "dyn" | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override"
        | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "try" => {
            Ident::new_raw(name, Span::call_site())
        }
        _ => Ident::new(name, Span::call_site()),
    }
}

/// Generates code to set a field in a protobuf message struct.
fn generate_setter(field_path: &str, is_repeated: bool, field_type: Option<i32>) -> TokenStream {
    let parts: Vec<&str> = field_path.split('.').collect();
    let val_ident = quote! { val };

    // TYPE_BYTES = 12
    let is_bytes = field_type == Some(12);

    if parts.len() == 1 {
        let field = safe_ident(&parts[0].to_snake_case());
        if is_repeated {
            if is_bytes {
                quote! { proto_req.#field.push(#val_ident.as_bytes().to_vec()); }
            } else {
                quote! { proto_req.#field.push(#val_ident.parse().unwrap_or_default()); }
            }
        } else {
            if is_bytes {
                quote! { proto_req.#field = #val_ident.as_bytes().to_vec(); }
            } else {
                quote! { proto_req.#field = #val_ident.parse().unwrap_or_default(); }
            }
        }
    } else {
        let mut access = quote! { proto_req };
        for (i, part) in parts.iter().enumerate() {
            let field = safe_ident(&part.to_snake_case());
            if i == parts.len() - 1 {
                if is_repeated {
                    if is_bytes {
                        return quote! { #access.#field.push(#val_ident.as_bytes().to_vec()); };
                    } else {
                        return quote! { #access.#field.push(#val_ident.parse().unwrap_or_default()); };
                    }
                } else {
                    if is_bytes {
                        return quote! { #access.#field = #val_ident.as_bytes().to_vec(); };
                    } else {
                        return quote! { #access.#field = #val_ident.parse().unwrap_or_default(); };
                    }
                }
            } else {
                access =
                    quote! { #access.#field.get_or_insert_with(::core::default::Default::default) };
            }
        }
        quote! {}
    }
}

/// Generates the Rust code for a gRPC service registration.
pub fn generate_service(service: &ServiceDefinition, options: &GeneratorOptions) -> TokenStream {
    let registration_struct_name = format_ident!("{}Registration", service.name.to_pascal_case());
    let register_fn_name = format_ident!("register_{}", service.name.to_snake_case());

    let client_type = {
        let svc_client_mod = format_ident!("{}_client", service.name.to_snake_case());
        let svc_client = format_ident!("{}Client", service.name.to_pascal_case());

        if options.source_relative {
            // "dependency paths must be super::"
            quote! { super::#svc_client_mod::#svc_client<gateway_runtime::tonic::transport::Channel> }
        } else {
            let pkg = resolve_type(&service.package);
            quote! { #pkg::#svc_client_mod::#svc_client<gateway_runtime::tonic::transport::Channel> }
        }
    };

    let service_docs = service.docs.iter().map(|line| quote! { #[doc = #line] });

    // Generate service methods
    let methods: Vec<TokenStream> = service.methods.iter().map(|method| {
        let method_name = safe_ident(&method.name.to_snake_case());

        let input_type = resolve_relative_type(
            &method.input_type,
            &service.package,
            options,
        );

        #[allow(unused_variables)]
        let output_type = resolve_relative_type(
            &method.output_type,
            &service.package,
            options,
        );

        let method_docs = if method.docs.is_empty() {
            let footer = format!(
                "This endpoint is generated from {}.{} defined in {}.",
                method.proto_service, method.name, method.proto_file
            );
            quote! { #[doc = #footer] }
        } else {
            let lines = method.docs.iter().map(|line| quote! { #[doc = #line] });
            quote! { #( #lines )* }
        };

        let forward_call = if method.server_streaming {
            quote! {
                gateway_runtime::forward::forward_response_stream(codec, resp.into_inner(), req).await
            }
        } else {
            quote! {
                gateway_runtime::forward::forward_response_message(codec, &resp.into_inner(), req)
            }
        };

        let resp_type_annotation = if method.server_streaming {
            quote! { ::gateway_runtime::tonic::Response<::gateway_runtime::tonic::Streaming<_>> }
        } else {
            quote! { ::gateway_runtime::tonic::Response<_> }
        };

        quote! {
            #method_docs
            #[doc = ""]
            pub async fn #method_name<C>(
                &self,
                client: &mut #client_type,
                codec: &C,
                proto_req: #input_type,
                req: &::gateway_runtime::GatewayRequest
            ) -> ::gateway_runtime::GatewayResult
            where
                C: ::gateway_runtime::codec::Codec + Send + Sync + 'static + Clone,
            {
                use ::gateway_runtime::codec::Codec;

                let mut tonic_req = ::gateway_runtime::tonic::Request::new(proto_req);
                ::gateway_runtime::metadata::forward_metadata(req, tonic_req.metadata_mut());

                if let Some(timeout_str) = req.headers().get("grpc-timeout").and_then(|h| h.to_str().ok()) {
                    if let Some(duration) = ::gateway_runtime::metadata::grpc_timeout(timeout_str) {
                        tonic_req.set_timeout(duration);
                    }
                }

                let resp: #resp_type_annotation = match client.#method_name(tonic_req).await {
                    Ok(r) => r,
                    Err(e) => return Err(::gateway_runtime::errors::GatewayError::Upstream(e)),
                };

                #forward_call
            }

        }
    }).collect();

    // Generate registration logic
    let mut registration_logic: Vec<TokenStream> = Vec::new();
    for method in &service.methods {
        let method_name = format_ident!("{}", method.name.to_snake_case());
        let input_type = resolve_relative_type(&method.input_type, &service.package, options);

        for binding in &method.bindings {
            let http_method_ident = format_ident!("{}", binding.http_method);
            let pattern = path_compiler::compile(&binding.path_tmpl.template);

            let ops_tokens: Vec<TokenStream> = pattern.ops.iter().map(|op| {
                let code_ident = match op.code {
                    OpCode::Nop => quote! { gateway_internal::path_template::OpCode::Nop },
                    OpCode::Push => quote! { gateway_internal::path_template::OpCode::Push },
                    OpCode::LitPush => quote! { gateway_internal::path_template::OpCode::LitPush },
                    OpCode::PushM => quote! { gateway_internal::path_template::OpCode::PushM },
                    OpCode::ConcatN => quote! { gateway_internal::path_template::OpCode::ConcatN },
                    OpCode::Capture => quote! { gateway_internal::path_template::OpCode::Capture },
                };
                let operand = op.operand;
                quote! { gateway_internal::path_template::Op { code: #code_ident, operand: #operand } }
            }).collect();

            let pool_tokens: Vec<TokenStream> = pattern
                .pool
                .iter()
                .map(|s| quote! { #s.to_string() })
                .collect();
            let vars_tokens: Vec<TokenStream> = pattern
                .vars
                .iter()
                .map(|s| quote! { #s.to_string() })
                .collect();
            let stack_size = pattern.stack_size;
            let tail_len = pattern.tail_len;
            let verb_token = match &pattern.verb {
                Some(v) => quote! { Some(#v.to_string()) },
                None => quote! { None },
            };

            let population_logic: Vec<TokenStream> = binding
                .path_params
                .iter()
                .map(|param| {
                    let field_path = &param.field_path;
                    let is_repeated = param.is_repeated;
                    let field_type = param.field_type;
                    let setter = generate_setter(field_path, is_repeated, field_type);
                    quote! {
                        if let Some(val) = params.get(#field_path) {
                            #setter
                        }
                    }
                })
                .collect();

            let params_ident = if binding.path_params.is_empty() {
                quote! { _params }
            } else {
                quote! { params }
            };
            let mut_token = if binding.path_params.is_empty() {
                quote! {}
            } else {
                quote! { mut }
            };

            let unmarshal_logic = if let Some(body) = &binding.body {
                if body.is_whole_message() {
                    quote! {
                        match gateway_runtime::utilities::parse_body(&parts.headers, body, &codec).await {
                            Ok(v) => v,
                            Err(e) => return Ok(gateway_runtime::errors::handle_error(::gateway_runtime::tonic::Status::invalid_argument(e.to_string()))),
                        }
                    }
                } else {
                    quote! { #input_type::default() }
                }
            } else {
                quote! { #input_type::default() }
            };

            registration_logic.push(quote! {
                {
                    let client = client.clone();
                    let codec = codec.clone();

                    let pattern = gateway_internal::path_template::Pattern {
                        ops: vec![ #( #ops_tokens ),* ],
                        pool: vec![ #( #pool_tokens ),* ],
                        vars: vec![ #( #vars_tokens ),* ],
                        stack_size: #stack_size,
                        tail_len: #tail_len,
                        verb: #verb_token,
                    };

                    let service = gateway_runtime::tower::service_fn(move |req: ::gateway_runtime::GatewayRequest| {
                        let mut client = client.clone();
                        let codec = codec.clone();
                        async move {
                            let (parts, body) = req.into_parts();

                            #[allow(unused_mut)]
                            let #mut_token proto_req: #input_type = #unmarshal_logic;

                            let req = ::gateway_runtime::http::Request::from_parts(parts, ::gateway_runtime::alloc::vec::Vec::new());

                            if let Some(#params_ident) = req.extensions().get::<::gateway_runtime::alloc::collections::BTreeMap<::gateway_runtime::alloc::string::String, ::gateway_runtime::alloc::string::String>>() {
                                #( #population_logic )*
                            }

                            #registration_struct_name.#method_name(&mut client, &codec, proto_req, &req).await
                        }
                    });

                    let boxed = gateway_runtime::tower::util::BoxCloneService::new(service);

                    ::gateway_runtime::router::route(
                        router,
                        ::gateway_runtime::http::Method::#http_method_ident,
                        pattern,
                        boxed
                    );
                }

            });
        }
    }

    let register_doc = format!(
        "Registers the `{}` service with the `Router`.",
        service.name
    );
    let register_doc_details = "This function routes HTTP requests matching the service's defined paths to the provided gRPC client, using the given codec for serialization.".to_string();

    quote! {
        #( #service_docs )*

        #[derive(Clone, Copy)]
        pub struct #registration_struct_name;

        impl #registration_struct_name {
            #[doc = #register_doc]
            #[doc = ""]
            #[doc = #register_doc_details]
            pub fn #register_fn_name<S, C>(
                router: &mut gateway_runtime::router::Router<S>,
                client: #client_type,
                codec: C,
            )
            where
                C: gateway_runtime::codec::Codec + Send + Sync + 'static + Clone,
                S: From<gateway_runtime::BoxedGatewayService>
            {
                #( #registration_logic )*
            }

            #( #methods )*
        }
    }
}

pub struct GrpcGatewayGenerator {
    pub no_include: bool,
    pub source_relative: bool,
    pub extern_paths: HashMap<String, String>,
    pub file_map:
        HashMap<String, gateway_annotations::google::protobuf_custom::FileDescriptorProto>,
}

impl GrpcGatewayGenerator {
    pub fn new(
        file_map: HashMap<
            String,
            gateway_annotations::google::protobuf_custom::FileDescriptorProto,
        >,
        extern_paths: HashMap<String, String>,
    ) -> Self {
        Self {
            no_include: false,
            source_relative: true,
            extern_paths,
            file_map,
        }
    }
}

impl Generator for GrpcGatewayGenerator {
    fn generate(&mut self, module_request_set: &ModuleRequestSet) -> Result {
        let options = GeneratorOptions {
            source_relative: self.source_relative,
            extern_paths: self.extern_paths.clone(),
        };

        // Initialize symbol registry from input files using the rich descriptors
        let protos: Vec<gateway_annotations::google::protobuf_custom::FileDescriptorProto> =
            self.file_map.values().cloned().collect();
        let registry = descriptor_processor::SymbolRegistry::new(&protos);

        module_request_set
            .requests()
            .filter_map(|(_module, request)| {
                let output_filename = format!("{}.gw.rs", request.proto_package_name());

                // Aggregate all files in this request (which maps to one package)
                let services: Vec<_> = request
                    .files()
                    .flat_map(|file| {
                        // Look up the rich file descriptor using the name
                        let file_name = file.name.as_deref().unwrap_or_default();

                        if let Some(file_custom) = self.file_map.get(file_name) {
                            match descriptor_processor::process_file(file_custom, &registry) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("Error processing file {}: {}", file_name, e);
                                    Vec::new()
                                }
                            }
                        } else {
                            eprintln!("Warning: Could not find rich descriptor for {}", file_name);
                            Vec::new()
                        }
                    })
                    .collect();

                if services.is_empty() {
                    return None;
                }

                let mut file_tokens = TokenStream::new();
                for svc in &services {
                    let service_tokens = generate_service(svc, &options);
                    let mod_name_str = format!("{}_gw", svc.name.to_snake_case());
                    let mod_name = Ident::new(&mod_name_str, Span::call_site());

                    file_tokens.extend(quote! {
                        pub mod #mod_name {
                            #![allow(clippy::all)]
                            #![allow(dead_code)]
                            #![allow(unused)]

                            #service_tokens
                        }
                    });
                }

                let final_tokens = quote! {
                    #file_tokens
                };

                let syntax_tree: syn::File = match syn::parse2(final_tokens) {
                    Ok(f) => f,
                    Err(e) => panic!("Failed to parse generated code: {}", e),
                };
                let formatted_content = prettyplease::unparse(&syntax_tree);
                let version = env!("CARGO_PKG_VERSION");
                let content = format!(
                    "// This file is @generated by protoc-gen-grpc-gateway-rust version-{}.\n\n{}",
                    version, formatted_content
                );

                let mut res = Vec::new();

                if !self.no_include {
                    if let Some(f) = request.append_to_file(|buf| {
                        buf.push_str("include!(\"");
                        buf.push_str(&output_filename);
                        buf.push_str("\");\n");
                    }) {
                        res.push(f);
                    }
                }

                let out_dir = request.output_dir();
                res.push(File {
                    name: Some(out_dir + &output_filename),
                    content: Some(content),
                    ..File::default()
                });

                Some(res)
            })
            .flatten()
            .map(Ok)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_resolve_relative_type_extern_path_exact() {
        let mut extern_paths = HashMap::new();
        extern_paths.insert(
            ".google.protobuf.Timestamp".to_string(),
            "::pbjson_types::Timestamp".to_string(),
        );

        let options = GeneratorOptions {
            source_relative: true,
            extern_paths,
        };

        let stream = resolve_relative_type(".google.protobuf.Timestamp", "my.pkg", &options);
        assert_eq!(stream.to_string(), ":: pbjson_types :: Timestamp");
    }

    #[test]
    fn test_resolve_relative_type_extern_path_prefix() {
        let mut extern_paths = HashMap::new();
        extern_paths.insert(".google.protobuf".to_string(), "::pbjson_types".to_string());

        let options = GeneratorOptions {
            source_relative: true,
            extern_paths,
        };

        let stream = resolve_relative_type(".google.protobuf.Timestamp", "my.pkg", &options);
        assert_eq!(stream.to_string(), ":: pbjson_types :: Timestamp");
    }

    #[test]
    fn test_resolve_relative_type_extern_path_prefix_nested() {
        let mut extern_paths = HashMap::new();
        extern_paths.insert(".my.common".to_string(), "::common_crate".to_string());

        let options = GeneratorOptions {
            source_relative: true,
            extern_paths,
        };

        let stream = resolve_relative_type(".my.common.Status", "my.pkg", &options);
        assert_eq!(stream.to_string(), ":: common_crate :: Status");
    }

    #[test]
    fn test_resolve_relative_type_no_match() {
        let options = GeneratorOptions {
            source_relative: true,
            extern_paths: HashMap::new(),
        };

        // Current package "my.pkg", target ".my.pkg.Foo" -> "Foo" (if source relative and same package)
        // resolve_relative_type logic for same package:
        // current_parts: ["my", "pkg"]
        // type_parts: ["my", "pkg", "Foo"]
        // common: 2
        // super_count: 2 - 2 + 1 = 1 (super::)
        // remaining type: Foo
        // Result: super::Foo

        let stream = resolve_relative_type(".my.pkg.Foo", "my.pkg", &options);
        assert_eq!(stream.to_string(), "super :: Foo");
    }
}
