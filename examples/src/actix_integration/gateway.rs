use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use gateway_examples::examplepb::a_bit_of_everything_service_client::ABitOfEverythingServiceClient;
use gateway_examples::gateway::ABitOfEverythingServiceRegistration;
use gateway_runtime::codec::JsonCodec;
use gateway_runtime::router::Router;
use gateway_runtime::utilities::SyncService;
use gateway_runtime::BoxedGatewayService;
use gateway_runtime::GatewayRequest;
use http_body_util::BodyExt;
use std::sync::Arc;
use tonic::transport::Channel;
use tower::Service;

// Handler that bridges Actix Web requests to the Gateway Router
async fn gateway_handler(
    req: actix_web::HttpRequest,
    body: web::Bytes,
    router: web::Data<Arc<Router<SyncService<BoxedGatewayService>>>>,
) -> impl Responder {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    println!("Actix Gateway Request: {} {}", method, path);

    // Convert Actix request (http 0.2) to GatewayRequest parts (http 1.x)
    let method_str = method.as_str();
    let method_http1 = http::Method::from_bytes(method_str.as_bytes()).unwrap();

    let uri_str = req.uri().to_string();
    let uri_http1 = http::Uri::try_from(uri_str).unwrap();

    let mut parts = http::request::Builder::new()
        .method(method_http1.clone())
        .uri(uri_http1)
        .body(())
        .unwrap()
        .into_parts()
        .0;

    // Copy headers
    for (k, v) in req.headers() {
        if let Ok(key) = http::header::HeaderName::from_bytes(k.as_str().as_bytes()) {
            if let Ok(val) = http::header::HeaderValue::from_bytes(v.as_bytes()) {
                parts.headers.insert(key, val);
            }
        }
    }

    if let Some((service, params)) = router.match_request(&method_http1, &path) {
        let mut service = service.get().clone();
        parts.extensions.insert(params);
        let gateway_req = GatewayRequest::from_parts(parts, body.to_vec());

        match service.call(gateway_req).await {
            Ok(resp) => {
                let (parts, body) = resp.into_parts();

                // Collect the body
                let bytes = match body.collect().await {
                    Ok(b) => b.to_bytes(),
                    Err(_) => return HttpResponse::InternalServerError().body("Body Error"),
                };

                // Convert http 1.0 status to actix (http 0.2) status
                let status_code = actix_web::http::StatusCode::from_u16(parts.status.as_u16()).unwrap();
                let mut builder = HttpResponse::build(status_code);

                // Copy headers back
                for (k, v) in parts.headers {
                    if let Some(key) = k {
                        if let Ok(actix_key) = actix_web::http::header::HeaderName::from_bytes(key.as_str().as_bytes()) {
                            if let Ok(actix_val) = actix_web::http::header::HeaderValue::from_bytes(v.as_bytes()) {
                                builder.insert_header((actix_key, actix_val));
                            }
                        }
                    }
                }
                builder.body(bytes)
            }
            Err(e) => {
                eprintln!("Gateway Error: {:?}", e);
                HttpResponse::InternalServerError().body(format!("Gateway Error: {:?}", e))
            }
        }
    } else {
        HttpResponse::NotFound().finish()
    }
}

async fn index(_req: actix_web::HttpRequest) -> impl Responder {
    "Hello World from Actix!"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 1. Dial gRPC
    let channel = Channel::from_static("http://127.0.0.1:9090")
        .connect()
        .await
        .expect("Failed to connect to gRPC server");

    let client = ABitOfEverythingServiceClient::new(channel);

    // 2. Setup Router
    let mut router = Router::<SyncService<BoxedGatewayService>>::new();
    let codec = JsonCodec;

    ABitOfEverythingServiceRegistration::register_a_bit_of_everything_service(
        &mut router,
        client,
        codec,
    );

    let router = Arc::new(router);

    println!("Actix Gateway listening on http://127.0.0.1:8082");

    // 3. Serve Actix
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(router.clone()))
            .route("/", web::get().to(index))
            .default_service(web::to(gateway_handler))
    })
        .bind(("127.0.0.1", 8082))?
        .run()
        .await
}
