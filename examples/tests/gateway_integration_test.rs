use gateway_examples::examplepb::a_bit_of_everything_service_client::ABitOfEverythingServiceClient;
use gateway_examples::examplepb::a_bit_of_everything_service_server::{
    ABitOfEverythingService, ABitOfEverythingServiceServer,
};
use gateway_examples::examples::internal::proto::sub::StringMessage;
use gateway_examples::examplepb::a_bit_of_everything_service_gw::ABitOfEverythingServiceRegistration;
use gateway_examples::google;
use gateway_runtime::codec::MultimediaCodec;
use gateway_runtime::router::Router;
use gateway_runtime::utilities::SyncService;
use gateway_runtime::{BoxedGatewayService, Gateway, GatewayRequest};
use http_body_util::BodyExt;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tonic::{transport::Server, Request, Response, Status};
use tower::Service;

#[derive(Debug, Default)]
pub struct MyService;

#[tonic::async_trait]
impl ABitOfEverythingService for MyService {
    async fn echo(
        &self,
        request: Request<StringMessage>,
    ) -> Result<Response<StringMessage>, Status> {
        let inner = request.into_inner();
        // Check for specific value to trigger error
        if inner.value == "trigger-error" {
            return Err(Status::not_found("triggered not found"));
        }

        Ok(Response::new(StringMessage { value: inner.value }))
    }

    // Implement other methods as unimplemented
    async fn create(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn create_body(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn create_book(
        &self,
        _request: Request<gateway_examples::examplepb::CreateBookRequest>,
    ) -> Result<Response<gateway_examples::examplepb::Book>, Status> {
        unimplemented!()
    }
    async fn update_book(
        &self,
        _request: Request<gateway_examples::examplepb::UpdateBookRequest>,
    ) -> Result<Response<gateway_examples::examplepb::Book>, Status> {
        unimplemented!()
    }
    async fn lookup(
        &self,
        _request: Request<gateway_examples::examples::internal::proto::sub2::IdMessage>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn custom(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn double_colon(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn update(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn update_v2(
        &self,
        _request: Request<gateway_examples::examplepb::UpdateV2Request>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn delete(
        &self,
        _request: Request<gateway_examples::examples::internal::proto::sub2::IdMessage>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn get_query(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn get_repeated_query(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverythingRepeated>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverythingRepeated>, Status> {
        unimplemented!()
    }
    async fn deep_path_echo(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn no_bindings(
        &self,
        _request: Request<google::protobuf::Duration>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn timeout(
        &self,
        _request: Request<google::protobuf::Empty>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn error_with_details(
        &self,
        _request: Request<google::protobuf::Empty>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn get_message_with_body(
        &self,
        _request: Request<gateway_examples::examplepb::MessageWithBody>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn post_with_empty_body(
        &self,
        _request: Request<gateway_examples::examplepb::Body>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn check_get_query_params(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn check_nested_enum_get_query_params(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn check_post_query_params(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn overwrite_request_content_type(
        &self,
        _request: Request<gateway_examples::examplepb::Body>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn overwrite_response_content_type(
        &self,
        _request: Request<google::protobuf::Empty>,
    ) -> Result<Response<google::protobuf::StringValue>, Status> {
        unimplemented!()
    }
    async fn check_external_path_enum(
        &self,
        _request: Request<
            gateway_examples::examples::internal::proto::pathenum::MessageWithPathEnum,
        >,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn check_external_nested_path_enum(
        &self,
        _request: Request<
            gateway_examples::examples::internal::proto::pathenum::MessageWithNestedPathEnum,
        >,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn check_status(
        &self,
        _request: Request<google::protobuf::Empty>,
    ) -> Result<Response<gateway_examples::examplepb::CheckStatusResponse>, Status> {
        unimplemented!()
    }
    async fn exists(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn custom_options_request(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn trace_request(
        &self,
        _request: Request<gateway_examples::examplepb::ABitOfEverything>,
    ) -> Result<Response<gateway_examples::examplepb::ABitOfEverything>, Status> {
        unimplemented!()
    }
    async fn post_oneof_enum(
        &self,
        _request: Request<gateway_examples::examples::internal::proto::oneofenum::OneofEnumMessage>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
    async fn post_required_message_type(
        &self,
        _request: Request<gateway_examples::examplepb::RequiredMessageTypeRequest>,
    ) -> Result<Response<google::protobuf::Empty>, Status> {
        unimplemented!()
    }
}

#[tokio::test]
async fn test_gateway_integration() {
    // 1. Start gRPC Server
    let grpc_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let grpc_listener = TcpListener::bind(grpc_addr).await.unwrap();
    let grpc_port = grpc_listener.local_addr().unwrap().port();
    let grpc_addr_str = format!("http://127.0.0.1:{}", grpc_port);

    tokio::spawn(async move {
        Server::builder()
            .add_service(ABitOfEverythingServiceServer::new(MyService))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Start Gateway Server with Gateway Runtime features
    let gateway_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let gateway_listener = TcpListener::bind(gateway_addr).await.unwrap();
    let gateway_port = gateway_listener.local_addr().unwrap().port();
    let gateway_url = format!("http://127.0.0.1:{}", gateway_port);

    tokio::spawn(async move {
        let channel = tonic::transport::Channel::from_shared(grpc_addr_str.clone())
            .unwrap()
            .connect()
            .await
            .unwrap();

        let client = ABitOfEverythingServiceClient::new(channel);
        let mut router = Router::<SyncService<BoxedGatewayService>>::new();
        let codec = MultimediaCodec::new();

        ABitOfEverythingServiceRegistration::register_a_bit_of_everything_service(
            &mut router,
            client,
            codec,
        );

        let gateway = Gateway::new(router)
            // defaults are already set for matchers, error handler, metadata.
            // We just append a custom response modifier.
            .with_response_modifier(|_req, resp| {
                resp.headers_mut()
                    .insert("x-gateway-processed", "true".parse().unwrap());
            })
            .with_unescaping_mode(gateway_runtime::UnescapingMode::AllCharacters)
            .into_service();

        loop {
            let (stream, _) = gateway_listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            let gateway_clone = gateway.clone();

            tokio::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                            let mut gateway = gateway_clone.clone();
                            async move {
                                let (parts, body) = req.into_parts();
                                let bytes = body.collect().await.unwrap().to_bytes();
                                let body_vec = bytes.to_vec();
                                let gateway_req = GatewayRequest::from_parts(parts, body_vec);

                                let resp = gateway.call(gateway_req).await.unwrap();
                                Ok::<_, std::convert::Infallible>(resp)
                            }
                        }),
                    )
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();

    // 3. Test Unescaping & Success Flow
    // GET /v1/example/a_bit_of_everything/echo/{value}
    // We also test that x-request-id is stripped (client provided) and replaced by a new one (response header from upstream/gateway?)
    // Actually, x-request-id is in metadata. The upstream echo doesn't necessarily return it in headers unless we reflect it.
    // The test mainly checks the request succeeds.
    let resp = client
        .get(format!(
            "{}/v1/example/a_bit_of_everything/echo/foo%20bar",
            gateway_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-gateway-processed").unwrap(), "true");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["value"], "foo bar");

    // 4. Test Error Handling (via value trigger)
    let resp = client
        .get(format!(
            "{}/v1/example/a_bit_of_everything/echo/trigger-error",
            gateway_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );

    let error_body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(error_body["status_code"], 404);
    assert_eq!(error_body["title"], "Error");
    assert!(error_body["message"]
        .as_str()
        .unwrap()
        .contains("triggered not found"));
}
