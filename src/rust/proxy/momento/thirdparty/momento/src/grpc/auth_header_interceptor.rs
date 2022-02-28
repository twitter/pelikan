#[derive(Clone)]
pub struct AuthHeaderInterceptor {
    pub auth_key: String,
}

impl tonic::service::Interceptor for AuthHeaderInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request.metadata_mut().insert(
            "authorization",
            tonic::metadata::AsciiMetadataValue::from_str(self.auth_key.as_str()).unwrap(),
        );
        Ok(request)
    }
}
