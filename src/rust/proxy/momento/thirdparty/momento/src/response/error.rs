use tonic::codegen::http;

#[derive(Debug)]
pub enum MomentoError {
    InternalServerError(String),
    BadRequest(String),
    PermissionDenied(String),
    Unauthenticated(String),
    NotFound(String),
    AlreadyExists(String),
    Cancelled(String),
    Timeout(String),
    LimitExceeded(String),
    ClientSdkError(String),
    InvalidArgument(String),
}

impl std::fmt::Display for MomentoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MomentoError::InternalServerError(e)
            | MomentoError::BadRequest(e)
            | MomentoError::PermissionDenied(e)
            | MomentoError::Unauthenticated(e)
            | MomentoError::NotFound(e)
            | MomentoError::AlreadyExists(e)
            | MomentoError::Cancelled(e)
            | MomentoError::Timeout(e)
            | MomentoError::LimitExceeded(e)
            | MomentoError::ClientSdkError(e)
            | MomentoError::InvalidArgument(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for MomentoError {}

impl From<http::uri::InvalidUri> for MomentoError {
    fn from(e: http::uri::InvalidUri) -> Self {
        // the uri gets derived from the jwt
        Self::ClientSdkError(e.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for MomentoError {
    fn from(_: jsonwebtoken::errors::Error) -> Self {
        let err_msg = "Failed to parse Auth Token".to_string();
        Self::ClientSdkError(err_msg)
    }
}

impl From<String> for MomentoError {
    fn from(s: String) -> Self {
        Self::BadRequest(s)
    }
}

impl From<tonic::transport::Error> for MomentoError {
    fn from(e: tonic::transport::Error) -> Self {
        Self::InternalServerError(e.to_string())
    }
}

impl From<tonic::Status> for MomentoError {
    fn from(s: tonic::Status) -> Self {
        status_to_error(s)
    }
}

fn status_to_error(status: tonic::Status) -> MomentoError {
    match status.code() {
        tonic::Code::InvalidArgument
        | tonic::Code::Unimplemented
        | tonic::Code::OutOfRange
        | tonic::Code::FailedPrecondition => MomentoError::BadRequest(status.message().to_string()),
        tonic::Code::Cancelled => MomentoError::Cancelled(status.message().to_string()),
        tonic::Code::DeadlineExceeded => MomentoError::Timeout(status.message().to_string()),
        tonic::Code::PermissionDenied => {
            MomentoError::PermissionDenied(status.message().to_string())
        }
        tonic::Code::Unauthenticated => MomentoError::Unauthenticated(status.message().to_string()),
        tonic::Code::ResourceExhausted => MomentoError::LimitExceeded(status.message().to_string()),
        tonic::Code::NotFound => MomentoError::NotFound(status.message().to_string()),
        tonic::Code::AlreadyExists => MomentoError::AlreadyExists(status.message().to_string()),
        tonic::Code::Unknown
        | tonic::Code::Aborted
        | tonic::Code::Internal
        | tonic::Code::Unavailable
        | tonic::Code::DataLoss
        | _ => MomentoError::InternalServerError(status.message().to_string()),
    }
}
