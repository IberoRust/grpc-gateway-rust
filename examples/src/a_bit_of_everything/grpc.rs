use gateway_examples::examplemultipart::stored_file_service_server::{
    StoredFileService, StoredFileServiceServer,
};
use gateway_examples::examplemultipart::{
    CreateFileRequest, DownloadStoredFileRequest, StoredFile,
};
use gateway_examples::examplepb::a_bit_of_everything_service_server::{
    ABitOfEverythingService, ABitOfEverythingServiceServer,
};
use gateway_examples::examplepb::camel_case_service_name_server::{
    CamelCaseServiceName, CamelCaseServiceNameServer,
};
use gateway_examples::examplepb::snake_enum_service_server::{
    SnakeEnumService, SnakeEnumServiceServer,
};
use gateway_examples::examplepb::{
    ABitOfEverything, ABitOfEverythingRepeated, Body, Book, CheckStatusResponse, CreateBookRequest,
    RequiredMessageTypeRequest, SnakeEnumRequest, SnakeEnumResponse, UpdateBookRequest,
    UpdateV2Request,
};
use gateway_examples::google::api::HttpBody;
use gateway_examples::google::protobuf::{Duration, Empty, StringValue};
use gateway_examples::grpc::gateway::examples::internal::proto::{
    oneofenum::OneofEnumMessage,
    pathenum::{MessageWithNestedPathEnum, MessageWithPathEnum},
    sub::StringMessage,
    sub2::IdMessage,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Default)]
pub struct MyABitOfEverythingService;

#[tonic::async_trait]
impl ABitOfEverythingService for MyABitOfEverythingService {
    async fn create(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Create): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (Create): {:?}", resp);
        Ok(resp)
    }

    async fn create_body(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CreateBody): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (CreateBody): {:?}", resp);
        Ok(resp)
    }

    async fn create_book(
        &self,
        request: Request<CreateBookRequest>,
    ) -> Result<Response<Book>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CreateBook): {:?}", inner);
        let resp = Response::new(Book::default());
        println!("gRPC Response (CreateBook): {:?}", resp);
        Ok(resp)
    }

    async fn update_book(
        &self,
        request: Request<UpdateBookRequest>,
    ) -> Result<Response<Book>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (UpdateBook): {:?}", inner);
        let resp = Response::new(Book::default());
        println!("gRPC Response (UpdateBook): {:?}", resp);
        Ok(resp)
    }

    async fn lookup(
        &self,
        request: Request<IdMessage>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Lookup): {:?}", inner);
        let resp = Response::new(ABitOfEverything::default());
        println!("gRPC Response (Lookup): {:?}", resp);
        Ok(resp)
    }

    async fn custom(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Custom): {:?}", inner);
        let resp = Response::new(ABitOfEverything::default());
        println!("gRPC Response (Custom): {:?}", resp);
        Ok(resp)
    }

    async fn double_colon(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (DoubleColon): {:?}", inner);
        let resp = Response::new(ABitOfEverything::default());
        println!("gRPC Response (DoubleColon): {:?}", resp);
        Ok(resp)
    }

    async fn update(&self, request: Request<ABitOfEverything>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Update): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (Update): {:?}", resp);
        Ok(resp)
    }

    async fn update_v2(
        &self,
        request: Request<UpdateV2Request>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (UpdateV2): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (UpdateV2): {:?}", resp);
        Ok(resp)
    }

    async fn delete(&self, request: Request<IdMessage>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Delete): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (Delete): {:?}", resp);
        Ok(resp)
    }

    async fn get_query(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (GetQuery): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (GetQuery): {:?}", resp);
        Ok(resp)
    }

    async fn get_repeated_query(
        &self,
        request: Request<ABitOfEverythingRepeated>,
    ) -> Result<Response<ABitOfEverythingRepeated>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (GetRepeatedQuery): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (GetRepeatedQuery): {:?}", resp);
        Ok(resp)
    }

    async fn echo(
        &self,
        request: Request<StringMessage>,
    ) -> Result<Response<StringMessage>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Echo): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (Echo): {:?}", resp);
        Ok(resp)
    }

    async fn deep_path_echo(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (DeepPathEcho): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (DeepPathEcho): {:?}", resp);
        Ok(resp)
    }

    async fn no_bindings(&self, request: Request<Duration>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (NoBindings): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (NoBindings): {:?}", resp);
        Ok(resp)
    }

    async fn timeout(&self, request: Request<Empty>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Timeout): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (Timeout): {:?}", resp);
        Ok(resp)
    }

    async fn error_with_details(&self, request: Request<Empty>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (ErrorWithDetails): {:?}", inner);
        println!("gRPC Response (ErrorWithDetails): Error");
        Err(Status::unknown("Error with details"))
    }

    async fn get_message_with_body(
        &self,
        request: Request<gateway_examples::examplepb::MessageWithBody>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (GetMessageWithBody): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (GetMessageWithBody): {:?}", resp);
        Ok(resp)
    }

    async fn post_with_empty_body(
        &self,
        request: Request<Body>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (PostWithEmptyBody): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (PostWithEmptyBody): {:?}", resp);
        Ok(resp)
    }

    async fn check_get_query_params(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CheckGetQueryParams): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (CheckGetQueryParams): {:?}", resp);
        Ok(resp)
    }

    async fn check_nested_enum_get_query_params(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CheckNestedEnumGetQueryParams): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (CheckNestedEnumGetQueryParams): {:?}", resp);
        Ok(resp)
    }

    async fn check_post_query_params(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CheckPostQueryParams): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (CheckPostQueryParams): {:?}", resp);
        Ok(resp)
    }

    async fn overwrite_request_content_type(
        &self,
        request: Request<Body>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (OverwriteRequestContentType): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (OverwriteRequestContentType): {:?}", resp);
        Ok(resp)
    }

    async fn overwrite_response_content_type(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<StringValue>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (OverwriteResponseContentType): {:?}", inner);
        let resp = Response::new(StringValue::default());
        println!("gRPC Response (OverwriteResponseContentType): {:?}", resp);
        Ok(resp)
    }

    async fn check_external_path_enum(
        &self,
        request: Request<MessageWithPathEnum>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CheckExternalPathEnum): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (CheckExternalPathEnum): {:?}", resp);
        Ok(resp)
    }

    async fn check_external_nested_path_enum(
        &self,
        request: Request<MessageWithNestedPathEnum>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CheckExternalNestedPathEnum): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (CheckExternalNestedPathEnum): {:?}", resp);
        Ok(resp)
    }

    async fn check_status(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<CheckStatusResponse>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CheckStatus): {:?}", inner);
        let resp = Response::new(CheckStatusResponse::default());
        println!("gRPC Response (CheckStatus): {:?}", resp);
        Ok(resp)
    }

    async fn exists(&self, request: Request<ABitOfEverything>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (Exists): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (Exists): {:?}", resp);
        Ok(resp)
    }

    async fn custom_options_request(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CustomOptionsRequest): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (CustomOptionsRequest): {:?}", resp);
        Ok(resp)
    }

    async fn trace_request(
        &self,
        request: Request<ABitOfEverything>,
    ) -> Result<Response<ABitOfEverything>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (TraceRequest): {:?}", inner);
        let resp = Response::new(inner);
        println!("gRPC Response (TraceRequest): {:?}", resp);
        Ok(resp)
    }

    async fn post_oneof_enum(
        &self,
        request: Request<OneofEnumMessage>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (PostOneofEnum): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (PostOneofEnum): {:?}", resp);
        Ok(resp)
    }

    async fn post_required_message_type(
        &self,
        request: Request<RequiredMessageTypeRequest>,
    ) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (PostRequiredMessageType): {:?}", inner);
        let resp = Response::new(Empty::default());
        println!("gRPC Response (PostRequiredMessageType): {:?}", resp);
        Ok(resp)
    }
}

#[derive(Debug, Default)]
pub struct MyCamelCaseServiceName;

#[tonic::async_trait]
impl CamelCaseServiceName for MyCamelCaseServiceName {
    async fn empty(&self, request: Request<Empty>) -> Result<Response<Empty>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (CamelCase.Empty): {:?}", inner);
        Ok(Response::new(Empty::default()))
    }
}

#[derive(Debug, Default)]
pub struct MySnakeEnumService;

#[tonic::async_trait]
impl SnakeEnumService for MySnakeEnumService {
    async fn snake_enum(
        &self,
        request: Request<SnakeEnumRequest>,
    ) -> Result<Response<SnakeEnumResponse>, Status> {
        let inner = request.into_inner();
        println!("gRPC Request (SnakeEnum): {:?}", inner);
        Ok(Response::new(SnakeEnumResponse::default()))
    }
}

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
        println!("gRPC Request (CreateStoredFile): filename={}, content_len={}", inner.filename, inner.content.len());

        // Emulate creating a stored file
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
        println!("gRPC Request (DownloadStoredFile): {:?}", inner);

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:9090".parse()?;
    let service = MyABitOfEverythingService::default();
    let camel_service = MyCamelCaseServiceName::default();
    let snake_service = MySnakeEnumService::default();
    let stored_file_service = MyStoredFileService::default();

    println!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(ABitOfEverythingServiceServer::new(service))
        .add_service(CamelCaseServiceNameServer::new(camel_service))
        .add_service(SnakeEnumServiceServer::new(snake_service))
        .add_service(StoredFileServiceServer::new(stored_file_service))
        .serve(addr)
        .await?;

    Ok(())
}
