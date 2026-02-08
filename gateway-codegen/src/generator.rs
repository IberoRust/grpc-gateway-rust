//! # Generator
//!
//! ## Purpose
//! Contains the core logic for generating Rust source code from the processed service definitions.
//! It constructs the AST (Abstract Syntax Tree) using `quote` and emits the final output.
//!
//! ## Scope
//! This module provides:
//! -   `generate_service`: The main function that produces the code for a single gRPC service.
//! -   Helper functions for resolving types, safe identifiers, and field setters.
//!
//! ## Position in the Architecture
//! Called by `main.rs` after the `FileDescriptorSet` has been processed by `descriptor_processor`.
//! It takes `ServiceDefinition` structs and outputs `proc_macro2::TokenStream`s.
//!
//! ## Design Constraints
//! -   **Code Correctness**: Generated code must compile and correctly use `gateway-runtime`.
//! -   **Hygiene**: Uses `quote` to ensure proper scoping and avoid identifier collisions.
//! -   **Formatting**: Output tokens are formatted later by `prettyplease`.

use crate::descriptor_processor::ServiceDefinition;
use crate::path_compiler;
use gateway_internal::path_template::OpCode;
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};


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
/// Methods, docs, and registration logic are separated with newlines to ensure
/// proper formatting after prettyplease.
pub fn generate_service(service: &ServiceDefinition) -> TokenStream {
    let registration_struct_name = format_ident!("{}Registration", service.name.to_pascal_case());
    let register_fn_name = format_ident!("register_{}", service.name.to_snake_case());

    let client_type = {
        let pkg = resolve_type(&service.package);
        let svc_client_mod = format_ident!("{}_client", service.name.to_snake_case());
        let svc_client = format_ident!("{}Client", service.name.to_pascal_case());
        quote! { #pkg::#svc_client_mod::#svc_client<gateway_runtime::tonic::transport::Channel> }
    };

    let service_docs = service.docs.iter().map(|line| quote! { #[doc = #line] });

    // Generate service methods with separation
    let methods: Vec<TokenStream> = service.methods.iter().map(|method| {
        let method_name = safe_ident(&method.name.to_snake_case());
        let input_type = resolve_type(&method.input_type);
        #[allow(unused_variables)]
        let output_type = resolve_type(&method.output_type);

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
                gateway_runtime::forward::forward_response_stream(codec, resp.into_inner()).await
            }
        } else {
            quote! {
                gateway_runtime::forward::forward_response_message(codec, &resp.into_inner())
            }
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

                let resp = match client.#method_name(tonic_req).await {
                    Ok(r) => r,
                    Err(e) => return Err(::gateway_runtime::errors::GatewayError::Upstream(e)),
                };

                #forward_call
            }

        }
    }).collect();

    // Generate registration logic with separation
    let mut registration_logic: Vec<TokenStream> = Vec::new();
    for method in &service.methods {
        let method_name = format_ident!("{}", method.name.to_snake_case());
        let input_type = resolve_type(&method.input_type);

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

            let pool_tokens: Vec<TokenStream> = pattern.pool.iter().map(|s| quote! { #s.to_string() }).collect();
            let vars_tokens: Vec<TokenStream> = pattern.vars.iter().map(|s| quote! { #s.to_string() }).collect();
            let stack_size = pattern.stack_size;
            let tail_len = pattern.tail_len;
            let verb_token = match &pattern.verb {
                Some(v) => quote! { Some(#v.to_string()) },
                None => quote! { None },
            };

            let population_logic: Vec<TokenStream> = binding.path_params.iter().map(|param| {
                let field_path = &param.field_path;
                let is_repeated = param.is_repeated;
                let field_type = param.field_type;
                let setter = generate_setter(field_path, is_repeated, field_type);
                quote! {
                    if let Some(val) = params.get(#field_path) {
                        #setter
                    }
                }
            }).collect();

            let params_ident = if binding.path_params.is_empty() { quote! { _params } } else { quote! { params } };
            let mut_token = if binding.path_params.is_empty() { quote! {} } else { quote! { mut } };

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

            }); // <- newline here ensures separation
        }
    }

    let register_doc = format!("Registers the `{}` service with the `Router`.", service.name);
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

            #( #methods )*  // <- methods are already separated by newlines in map above
        }
    }
}