use gateway_examples::examplemultipart::stored_file_service_client::StoredFileServiceClient;
use gateway_examples::examplemultipart::stored_file_service_server::{
    StoredFileService, StoredFileServiceServer,
};
use gateway_examples::examplemultipart::{
    CreateFileRequest, DownloadStoredFileRequest, StoredFile,
};
use gateway_examples::gateway::StoredFileServiceRegistration;
use gateway_examples::google::api::HttpBody;

use gateway_runtime::codec::MultimediaCodec;
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
use reqwest::multipart;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Default)]
pub struct MyStoredFileService;

#[tonic::async_trait]
impl StoredFileService for MyStoredFileService {
    type DownloadStoredFileStream = ReceiverStream<Result<HttpBody, Status>>;

    async fn create_stored_file(
        &self,
        request: Request<CreateFileRequest>,
    ) -> Result<Response<StoredFile>, Status> {
        let inner = request.into_inner();
        println!(
            "gRPC (Test) Request (CreateStoredFile): filename={}, content_len={}",
            inner.filename,
            inner.content.len()
        );

        Ok(Response::new(StoredFile {
            original_file_name: inner.filename,
            mime_type: inner.content_type,
            size_bytes: inner.content.len() as i64,
            file_storage_name: "stored-uuid-123".to_string(),
            file_path: "/tmp/stored-uuid-123".to_string(),
            identifier: 12345,
        }))
    }

    async fn download_stored_file(
        &self,
        request: Request<DownloadStoredFileRequest>,
    ) -> Result<Response<Self::DownloadStoredFileStream>, Status> {
        let inner = request.into_inner();
        println!("gRPC (Test) Request (DownloadStoredFile): {:?}", inner);

        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            let data = b"Hello, streamed world!";
            let chunk_size = 5;

            for chunk in data.chunks(chunk_size) {
                let body = HttpBody {
                    content_type: "text/plain".to_string(),
                    data: chunk.to_vec(),
                    extensions: vec![],
                };
                if tx.send(Ok(body)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::test]
async fn test_multipart_upload_and_download() {
    // 1. Start gRPC Server
    let grpc_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let grpc_listener = TcpListener::bind(grpc_addr).await.unwrap();
    let grpc_port = grpc_listener.local_addr().unwrap().port();
    let grpc_addr_str = format!("http://127.0.0.1:{}", grpc_port);

    let service = MyStoredFileService::default();

    tokio::spawn(async move {
        Server::builder()
            .add_service(StoredFileServiceServer::new(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
                grpc_listener,
            ))
            .await
            .unwrap();
    });

    // Wait for gRPC server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Start Gateway Server
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

        let client = StoredFileServiceClient::new(channel);
        let mut router = Router::<SyncService<BoxedGatewayService>>::new();
        let codec = MultimediaCodec::new();

        StoredFileServiceRegistration::register_stored_file_service(&mut router, client, codec);

        let router = Arc::new(router);

        loop {
            let (stream, _) = gateway_listener.accept().await.unwrap();
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
                                let bytes = body.collect().await.unwrap().to_bytes();
                                let body_vec = bytes.to_vec();

                                let method = parts.method.clone();
                                let path = parts.uri.path().to_string();

                                if let Some((service, params)) =
                                    router.match_request(&method, &path)
                                {
                                    let mut service = service.get().clone();
                                    parts.extensions.insert(params);
                                    let gateway_req = GatewayRequest::from_parts(parts, body_vec);

                                    let resp = service.call(gateway_req).await.unwrap();
                                    Ok::<_, std::convert::Infallible>(resp)
                                } else {
                                    Ok(hyper::Response::builder()
                                        .status(hyper::StatusCode::NOT_FOUND)
                                        .body(BodyExt::boxed_unsync(
                                            Full::new(Bytes::new()).map_err(|_| unreachable!()),
                                        ))
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
    });

    // Wait for Gateway server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 3. Test Multipart Upload
    let client = reqwest::Client::new();
    let form = multipart::Form::new().text("filename", "test.txt").part(
        "content",
        multipart::Part::bytes(b"Hello World".to_vec())
            .file_name("test.txt")
            .mime_str("text/plain")
            .unwrap(),
    );

    let resp = client
        .post(format!("{}/v1/example/files", gateway_url))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    println!("Upload Response: {:?}", json);
    assert_eq!(json["original_file_name"], "test.txt");
    assert_eq!(json["size_bytes"], 11); // integer because standard serde serialization without protojson options uses number

    // 4. Test Download Stream
    let resp = client
        .get(format!(
            "{}/v1/example/123/files/download/stored-uuid-123",
            gateway_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    // Since we stream HttpBody messages which are JSON encoded by default codec
    // We expect a stream of JSON objects or just bytes if we configured raw handling.
    // Currently Gateway emits JSON objects for each HttpBody message.
    // Let's inspect the content.
    let content = resp.text().await.unwrap();
    println!("Download Content: {}", content);

    // In current implementation, `forward_response_stream` uses `codec.encode(msg)`.
    // Since we use JsonCodec, it encodes `HttpBody` message to JSON.
    // So we expect multiple JSON objects concatenated (ndjson style if newline delimited, or just concatenated bytes).
    // JsonCodec output for one message is `{"contentType":"...","data":"..."}`.
    // `forward_response_stream` concatenates them.
    // Since JSON objects are not delimited by default in JsonCodec (no newline), this might be invalid JSON if concatenated directly.
    // However, `forward_response_stream` just extends buffer.
    // So `{"..."}{"..."}`.

    // Check for byte array representation of "Hello" [72,101,108,108,111]
    assert!(content.contains("[72,101,108,108,111]"));
    // Check for byte array representation of "streamed" (partial)
    // "strea" -> [115,116,114,101,97]
    assert!(content.contains("115,116,114"));
}
