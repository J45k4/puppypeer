#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FsChange {
    #[prost(string, tag = "1")]
    pub file_path: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub file_hash: ::prost::alloc::string::String,
    #[prost(uint64, tag = "3")]
    pub file_size: u64,
    #[prost(uint64, tag = "4")]
    pub credted_at: u64,
    #[prost(uint64, tag = "5")]
    pub modified_at: u64,
    #[prost(uint64, tag = "6")]
    pub accessed_at: u64,
    #[prost(bool, tag = "7")]
    pub readonly: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PushFsChangesRequest {
    #[prost(message, repeated, tag = "1")]
    pub changes: ::prost::alloc::vec::Vec<FsChange>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PushFsChangesResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeToCommandsRequest {
    #[prost(string, tag = "1")]
    pub agent_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ScanFolder {
    #[prost(string, tag = "1")]
    pub path: ::prost::alloc::string::String,
    #[prost(bool, tag = "2")]
    pub recursive: bool,
    #[prost(bool, tag = "3")]
    pub watch: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoveFiles {
    #[prost(string, tag = "1")]
    pub src_path: ::prost::alloc::string::String,
    #[prost(bool, tag = "2")]
    pub recursive: bool,
    #[prost(string, tag = "3")]
    pub dst_path: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UploadFile {
    #[prost(string, tag = "1")]
    pub file_hash: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Command {
    #[prost(oneof = "command::CommandType", tags = "1, 2, 3, 4, 5")]
    pub command_type: ::core::option::Option<command::CommandType>,
}
/// Nested message and enum types in `Command`.
pub mod command {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum CommandType {
        #[prost(message, tag = "1")]
        ScanFolder(super::ScanFolder),
        #[prost(message, tag = "2")]
        MoveFiles(super::MoveFiles),
        #[prost(message, tag = "3")]
        UploadFile(super::UploadFile),
        #[prost(bool, tag = "4")]
        RemoveMe(bool),
        #[prost(bool, tag = "5")]
        Ping(bool),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendClientInfoRequest {
    #[prost(string, tag = "1")]
    pub version: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SendClientInfoResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchFileMetadataRequest {
    #[prost(string, tag = "1")]
    pub file_hash: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchFileMetadataResponse {
    #[prost(string, tag = "1")]
    pub file_hash: ::prost::alloc::string::String,
    #[prost(uint64, tag = "2")]
    pub file_size: u64,
    #[prost(uint64, tag = "3")]
    pub credted_at: u64,
    #[prost(uint64, tag = "4")]
    pub modified_at: u64,
    #[prost(uint64, tag = "5")]
    pub accessed_at: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerEvent {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientEvent {}
#[doc = r" Generated client implementations."]
pub mod epic_shelter_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct EpicShelterClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl EpicShelterClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> EpicShelterClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn events(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::ClientEvent>,
        ) -> Result<tonic::Response<tonic::codec::Streaming<super::ServerEvent>>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/EpicShelter.EpicShelter/events");
            self.inner
                .streaming(request.into_streaming_request(), path, codec)
                .await
        }
        pub async fn push_fs_changes(
            &mut self,
            request: impl tonic::IntoRequest<super::PushFsChangesRequest>,
        ) -> Result<tonic::Response<super::PushFsChangesResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/EpicShelter.EpicShelter/push_fs_changes");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn subscribe_to_commands(
            &mut self,
            request: impl tonic::IntoRequest<super::SubscribeToCommandsRequest>,
        ) -> Result<tonic::Response<tonic::codec::Streaming<super::Command>>, tonic::Status>
        {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/EpicShelter.EpicShelter/subscribe_to_commands",
            );
            self.inner
                .server_streaming(request.into_request(), path, codec)
                .await
        }
        pub async fn send_client_info(
            &mut self,
            request: impl tonic::IntoRequest<super::SendClientInfoRequest>,
        ) -> Result<tonic::Response<super::SendClientInfoResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/EpicShelter.EpicShelter/send_client_info");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn fetch_file_metadata(
            &mut self,
            request: impl tonic::IntoRequest<super::FetchFileMetadataRequest>,
        ) -> Result<tonic::Response<super::FetchFileMetadataResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/EpicShelter.EpicShelter/fetch_file_metadata",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for EpicShelterClient<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }
    impl<T> std::fmt::Debug for EpicShelterClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "EpicShelterClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod epic_shelter_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with EpicShelterServer."]
    #[async_trait]
    pub trait EpicShelter: Send + Sync + 'static {
        #[doc = "Server streaming response type for the events method."]
        type eventsStream: Stream<Item = Result<super::ServerEvent, tonic::Status>>
            + Send
            + Sync
            + 'static;
        async fn events(
            &self,
            request: tonic::Request<tonic::Streaming<super::ClientEvent>>,
        ) -> Result<tonic::Response<Self::eventsStream>, tonic::Status>;
        async fn push_fs_changes(
            &self,
            request: tonic::Request<super::PushFsChangesRequest>,
        ) -> Result<tonic::Response<super::PushFsChangesResponse>, tonic::Status>;
        #[doc = "Server streaming response type for the subscribe_to_commands method."]
        type subscribe_to_commandsStream: Stream<Item = Result<super::Command, tonic::Status>>
            + Send
            + Sync
            + 'static;
        async fn subscribe_to_commands(
            &self,
            request: tonic::Request<super::SubscribeToCommandsRequest>,
        ) -> Result<tonic::Response<Self::subscribe_to_commandsStream>, tonic::Status>;
        async fn send_client_info(
            &self,
            request: tonic::Request<super::SendClientInfoRequest>,
        ) -> Result<tonic::Response<super::SendClientInfoResponse>, tonic::Status>;
        async fn fetch_file_metadata(
            &self,
            request: tonic::Request<super::FetchFileMetadataRequest>,
        ) -> Result<tonic::Response<super::FetchFileMetadataResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct EpicShelterServer<T: EpicShelter> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: EpicShelter> EpicShelterServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T, B> Service<http::Request<B>> for EpicShelterServer<T>
    where
        T: EpicShelter,
        B: HttpBody + Send + Sync + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/EpicShelter.EpicShelter/events" => {
                    #[allow(non_camel_case_types)]
                    struct eventsSvc<T: EpicShelter>(pub Arc<T>);
                    impl<T: EpicShelter> tonic::server::StreamingService<super::ClientEvent> for eventsSvc<T> {
                        type Response = super::ServerEvent;
                        type ResponseStream = T::eventsStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::ClientEvent>>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).events(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = eventsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/EpicShelter.EpicShelter/push_fs_changes" => {
                    #[allow(non_camel_case_types)]
                    struct push_fs_changesSvc<T: EpicShelter>(pub Arc<T>);
                    impl<T: EpicShelter> tonic::server::UnaryService<super::PushFsChangesRequest>
                        for push_fs_changesSvc<T>
                    {
                        type Response = super::PushFsChangesResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::PushFsChangesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).push_fs_changes(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = push_fs_changesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/EpicShelter.EpicShelter/subscribe_to_commands" => {
                    #[allow(non_camel_case_types)]
                    struct subscribe_to_commandsSvc<T: EpicShelter>(pub Arc<T>);
                    impl<T: EpicShelter>
                        tonic::server::ServerStreamingService<super::SubscribeToCommandsRequest>
                        for subscribe_to_commandsSvc<T>
                    {
                        type Response = super::Command;
                        type ResponseStream = T::subscribe_to_commandsStream;
                        type Future =
                            BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SubscribeToCommandsRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).subscribe_to_commands(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1;
                        let inner = inner.0;
                        let method = subscribe_to_commandsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/EpicShelter.EpicShelter/send_client_info" => {
                    #[allow(non_camel_case_types)]
                    struct send_client_infoSvc<T: EpicShelter>(pub Arc<T>);
                    impl<T: EpicShelter> tonic::server::UnaryService<super::SendClientInfoRequest>
                        for send_client_infoSvc<T>
                    {
                        type Response = super::SendClientInfoResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SendClientInfoRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).send_client_info(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = send_client_infoSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/EpicShelter.EpicShelter/fetch_file_metadata" => {
                    #[allow(non_camel_case_types)]
                    struct fetch_file_metadataSvc<T: EpicShelter>(pub Arc<T>);
                    impl<T: EpicShelter>
                        tonic::server::UnaryService<super::FetchFileMetadataRequest>
                        for fetch_file_metadataSvc<T>
                    {
                        type Response = super::FetchFileMetadataResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::FetchFileMetadataRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).fetch_file_metadata(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = fetch_file_metadataSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: EpicShelter> Clone for EpicShelterServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: EpicShelter> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: EpicShelter> tonic::transport::NamedService for EpicShelterServer<T> {
        const NAME: &'static str = "EpicShelter.EpicShelter";
    }
}
