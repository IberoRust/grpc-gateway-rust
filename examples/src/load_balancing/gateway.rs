use gateway_examples::examplepb::a_bit_of_everything_service_client::ABitOfEverythingServiceClient;
use gateway_examples::gateway::ABitOfEverythingServiceRegistration;
use gateway_runtime::codec::JsonCodec;
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
use std::time::Duration;
use tokio::net::TcpListener;
use tonic::transport::Channel;
use tonic::transport::Endpoint;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure Load Balanced Channel
    // List of gRPC server endpoints to balance between.
    // In a real scenario, these might come from service discovery (e.g., DNS, Consul).
    let endpoints = vec!["http://127.0.0.1:9090", "http://127.0.0.1:9091"];

    println!(
        "Creating load-balanced channel with endpoints: {:?}",
        endpoints
    );

    // Create a balanced channel using Round Robin (default behavior of balance_list)
    let channel = Channel::balance_list(
        endpoints
            .into_iter()
            .map(|e| Endpoint::from_static(e).timeout(Duration::from_secs(5))),
    );

    let client = ABitOfEverythingServiceClient::new(channel);

    // 2. Setup Router
    let mut router = Router::<SyncService<BoxedGatewayService>>::new();
    let codec = JsonCodec::new();

    // Register the service. The client internally handles load balancing.
    ABitOfEverythingServiceRegistration::register_a_bit_of_everything_service(
        &mut router,
        client,
        codec,
    );

    let router = Arc::new(router);

    // 3. Serve Gateway
    let addr: SocketAddr = "0.0.0.0:8081".parse()?;
    let listener = TcpListener::bind(addr).await?;
    println!("Load Balanced Gateway listening on {}", addr);

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
                            let (mut parts, body) = req.into_parts();
                            let bytes = match body.collect().await {
                                Ok(b) => b.to_bytes(),
                                Err(e) => {
                                    eprintln!("Error collecting body: {:?}", e);
                                    return Ok::<_, std::convert::Infallible>(
                                        http::Response::builder()
                                            .status(http::StatusCode::BAD_REQUEST)
                                            .body(BodyExt::boxed_unsync(
                                                Full::new(Bytes::from("Bad Request")).map_err(
                                                    |e| -> gateway_runtime::errors::GatewayError {
                                                        match e {}
                                                    },
                                                ),
                                            ))
                                            .unwrap(),
                                    );
                                }
                            };
                            let body_vec = bytes.to_vec();

                            let method = parts.method.clone();
                            let path = parts.uri.path().to_string();

                            println!("Load Balanced Request: {} {}", method, path);

                            if let Some((service, params, _meta)) =
                                router.match_request(&method, &path)
                            {
                                let mut service = service.get().clone();
                                parts.extensions.insert(params);
                                let gateway_req = GatewayRequest::from_parts(parts, body_vec);

                                let resp = match service.call(gateway_req).await {
                                    Ok(r) => r,
                                    Err(e) => {
                                        eprintln!("Gateway Error: {:?}", e);
                                        http::Response::builder()
                                            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(BodyExt::boxed_unsync(
                                                Full::new(Bytes::new()).map_err(
                                                    |e| -> gateway_runtime::errors::GatewayError {
                                                        match e {}
                                                    },
                                                ),
                                            ))
                                            .unwrap()
                                    }
                                };
                                Ok::<_, std::convert::Infallible>(resp)
                            } else {
                                Ok(http::Response::builder()
                                    .status(http::StatusCode::NOT_FOUND)
                                    .body(BodyExt::boxed_unsync(Full::new(Bytes::new()).map_err(
                                        |e| -> gateway_runtime::errors::GatewayError { match e {} },
                                    )))
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
