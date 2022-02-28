#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteCacheRequest {
    #[prost(string, tag = "1")]
    pub cache_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteCacheResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateCacheRequest {
    #[prost(string, tag = "1")]
    pub cache_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateCacheResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListCachesRequest {
    #[prost(string, tag = "1")]
    pub next_token: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Cache {
    #[prost(string, tag = "1")]
    pub cache_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListCachesResponse {
    #[prost(message, repeated, tag = "1")]
    pub cache: ::prost::alloc::vec::Vec<Cache>,
    #[prost(string, tag = "2")]
    pub next_token: ::prost::alloc::string::String,
}
#[doc = r" Generated client implementations."]
pub mod scs_control_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    #[derive(Debug, Clone)]
    pub struct ScsControlClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ScsControlClient<tonic::transport::Channel> {
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
    impl<T> ScsControlClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> ScsControlClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error:
                Into<StdError> + Send + Sync,
        {
            ScsControlClient::new(InterceptedService::new(inner, interceptor))
        }
        #[doc = r" Compress requests with `gzip`."]
        #[doc = r""]
        #[doc = r" This requires the server to support it otherwise it might respond with an"]
        #[doc = r" error."]
        pub fn send_gzip(mut self) -> Self {
            self.inner = self.inner.send_gzip();
            self
        }
        #[doc = r" Enable decompressing responses with `gzip`."]
        pub fn accept_gzip(mut self) -> Self {
            self.inner = self.inner.accept_gzip();
            self
        }
        pub async fn create_cache(
            &mut self,
            request: impl tonic::IntoRequest<super::CreateCacheRequest>,
        ) -> Result<tonic::Response<super::CreateCacheResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/control_client.ScsControl/CreateCache");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn delete_cache(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteCacheRequest>,
        ) -> Result<tonic::Response<super::DeleteCacheResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/control_client.ScsControl/DeleteCache");
            self.inner.unary(request.into_request(), path, codec).await
        }
        pub async fn list_caches(
            &mut self,
            request: impl tonic::IntoRequest<super::ListCachesRequest>,
        ) -> Result<tonic::Response<super::ListCachesResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/control_client.ScsControl/ListCaches");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
