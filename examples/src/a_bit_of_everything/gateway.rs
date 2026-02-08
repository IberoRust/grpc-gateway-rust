use clap::Parser;
use gateway_examples::examplemultipart::stored_file_service_client::StoredFileServiceClient;
use gateway_examples::examplepb::a_bit_of_everything_service_client::ABitOfEverythingServiceClient;
use gateway_examples::examplepb::camel_case_service_name_client::CamelCaseServiceNameClient;
use gateway_examples::examplepb::snake_enum_service_client::SnakeEnumServiceClient;
use gateway_examples::gateway::StoredFileServiceRegistration;
use gateway_examples::gateway::{
    ABitOfEverythingServiceRegistration, CamelCaseServiceNameRegistration,
    SnakeEnumServiceRegistration,
};

use gateway_runtime::codec::{JsonCodec, MultimediaCodec, ProtobufCodec};
use gateway_runtime::router::Router;
use gateway_runtime::tower::Service;
use gateway_runtime::utilities::SyncService;
use gateway_runtime::BoxedGatewayService;
use gateway_runtime::GatewayRequest;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tonic::transport::Channel;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:8080")]
    addr: String,
    #[arg(long, default_value = "http://localhost:9090")]
    endpoint: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // 1. Dial gRPC
    // Using from_shared needs a valid URI.
    let endpoint = args.endpoint.clone();
    let channel = Channel::from_shared(endpoint)?.connect().await?;

    let client = ABitOfEverythingServiceClient::new(channel.clone());
    let camel_client = CamelCaseServiceNameClient::new(channel.clone());
    let snake_client = SnakeEnumServiceClient::new(channel.clone());
    let stored_file_client = StoredFileServiceClient::new(channel.clone());

    // 2. Setup Router
    // We use SyncService to wrap BoxCloneService so it can be Sync and thus stored in Arc<Router>
    let mut router = Router::<SyncService<BoxedGatewayService>>::new();

    // Configure codecs
    let json_codec = JsonCodec::pretty();
    let proto_codec = ProtobufCodec;
    let codec = MultimediaCodec::with_codecs(json_codec, proto_codec);

    // The generated register function accepts S: From<BoxedGatewayService>.
    // SyncService<BoxedGatewayService> implements From<BoxedGatewayService>.
    ABitOfEverythingServiceRegistration::register_a_bit_of_everything_service(
        &mut router,
        client,
        codec.clone(),
    );
    CamelCaseServiceNameRegistration::register_camel_case_service_name(
        &mut router,
        camel_client,
        codec.clone(),
    );
    SnakeEnumServiceRegistration::register_snake_enum_service(
        &mut router,
        snake_client,
        codec.clone(),
    );

    StoredFileServiceRegistration::register_stored_file_service(
        &mut router,
        stored_file_client,
        codec.clone(),
    );

    let router = Arc::new(router);

    // 3. Serve
    let addr: SocketAddr = args.addr.parse()?;
    let listener = TcpListener::bind(addr).await?;
    println!("Gateway listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let router_clone = router.clone();

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                        let router = router_clone.clone();
                        async move {
                            // Collect body using http-body-util
                            let (mut parts, body) = req.into_parts();
                            let bytes = match body.collect().await {
                                Ok(b) => b.to_bytes(),
                                Err(e) => {
                                    eprintln!("Error collecting body: {:?}", e);
                                    let body = BodyExt::boxed_unsync(
                                        Full::new(Bytes::from("Bad Request")).map_err(
                                            |e| -> gateway_runtime::errors::GatewayError {
                                                match e {}
                                            },
                                        ),
                                    );
                                    return Ok::<_, std::convert::Infallible>(
                                        http::Response::builder()
                                            .status(http::StatusCode::BAD_REQUEST)
                                            .body(body)
                                            .unwrap(),
                                    );
                                }
                            };
                            let body_vec = bytes.to_vec();

                            let method = parts.method.clone();
                            let path = parts.uri.path().to_string();

                            println!(
                                "API Gateway Request: Method={} Path={} BodyLen={}",
                                method,
                                path,
                                body_vec.len()
                            );

                            if let Some((service, params)) = router.match_request(&method, &path) {
                                // Acquire the service from the Sync wrapper
                                let mut service = service.get().clone();

                                // Insert captured parameters into request extensions
                                parts.extensions.insert(params);

                                let gateway_req = GatewayRequest::from_parts(parts, body_vec);

                                let resp = match service.call(gateway_req).await {
                                    Ok(r) => r,
                                    Err(e) => {
                                        println!("Gateway Error: {:?}", e);
                                        let body =
                                            BodyExt::boxed_unsync(Full::new(Bytes::new()).map_err(
                                                |e| -> gateway_runtime::errors::GatewayError {
                                                    match e {}
                                                },
                                            ));
                                        http::Response::builder()
                                            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(body)
                                            .unwrap()
                                    }
                                };

                                Ok::<_, std::convert::Infallible>(resp)
                            } else {
                                println!("API Gateway Response: Status=404 (Not Found)");
                                let body = BodyExt::boxed_unsync(Full::new(Bytes::new()).map_err(
                                    |e| -> gateway_runtime::errors::GatewayError { match e {} },
                                ));
                                Ok(http::Response::builder()
                                    .status(http::StatusCode::NOT_FOUND)
                                    .body(body)
                                    .unwrap())
                            }
                        }
                    }),
                )
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
