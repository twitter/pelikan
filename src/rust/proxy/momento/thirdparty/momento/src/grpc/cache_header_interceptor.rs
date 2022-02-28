#[derive(Clone)]
pub struct CacheHeaderInterceptor {
    pub auth_key: String,
}

impl tonic::service::Interceptor for CacheHeaderInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        let request_metadata = request.metadata().clone();
        let mut result = tonic::Request::new(request.into_inner());
        result.metadata_mut().insert(
            "authorization",
            tonic::metadata::AsciiMetadataValue::from_str(self.auth_key.as_str()).unwrap(),
        );
        // for reasons unknown, tonic seems to be stripping out the content-type. So we need to add this as
        // a workaround so that the requests are successful
        result.metadata_mut().insert(
            "content-type",
            tonic::metadata::AsciiMetadataValue::from_str("application/grpc").unwrap(),
        );

        let cache_name = request_metadata.get("cache").unwrap();

        // need to re-add our `cache` header back into the interceptor or it will be stripped out
        result.metadata_mut().insert("cache", cache_name.clone());
        Ok(result)
    }
}
