pub mod google {
    pub mod api {
        tonic::include_proto!("google.api");
    }
    pub mod rpc {
        tonic::include_proto!("google.rpc");
    }
    pub mod protobuf {
        tonic::include_proto!("google.protobuf");
    }
}

pub mod grpc {
    pub mod gateway {
        pub mod protoc_gen_openapiv2 {
            pub mod options {
                tonic::include_proto!("grpc.gateway.protoc_gen_openapiv2.options");
            }
        }
        pub mod protoc_gen_openapiv3 {
            pub mod options {
                tonic::include_proto!("grpc.gateway.protoc_gen_openapiv3.options");
            }
        }
        pub mod examples {
            pub mod internal {
                pub mod proto {
                    pub mod examplepb {
                        tonic::include_proto!("grpc.gateway.examples.internal.proto.examplepb");
                        include!(concat!(
                        env!("OUT_DIR"),
                        "/gateway/grpc/gateway/examples/internal/proto/examplepb/grpc.gateway.examples.internal.proto.examplepb.gw.rs"
                        ));
                    }
                    pub mod sub {
                        tonic::include_proto!("grpc.gateway.examples.internal.proto.sub");
                    }
                    pub mod sub2 {
                        tonic::include_proto!("grpc.gateway.examples.internal.proto.sub2");
                    }
                    pub mod pathenum {
                        tonic::include_proto!("grpc.gateway.examples.internal.proto.pathenum");
                    }
                    pub mod oneofenum {
                        tonic::include_proto!("grpc.gateway.examples.internal.proto.oneofenum");
                    }
                    pub mod examplemultipar {
                        tonic::include_proto!(
                            "grpc.gateway.examples.internal.proto.examplemultipar"
                        );
                        include!(concat!(
                        env!("OUT_DIR"),
                        "/gateway/grpc/gateway/examples/internal/proto/examplemultipar/grpc.gateway.examples.internal.proto.examplemultipar.gw.rs"
                        ));
                    }
                }
            }
        }
    }
}

pub mod examplepb {
    pub use crate::grpc::gateway::examples::internal::proto::examplepb::*;
}

pub mod examples {
    pub mod internal {
        pub mod proto {
            pub use crate::grpc::gateway::examples::internal::proto::*;
        }
    }
}

pub mod examplemultipart {
    pub use crate::grpc::gateway::examples::internal::proto::examplemultipar::*;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_empty() {
        let e = crate::google::protobuf::Empty::default();
        let _ = serde_json::to_string(&e).unwrap();
    }
}
