#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Reply: {:?} {:?}", .0.code(), .0.message())]
    ServerError(#[from] tonic::Status),
    #[error("Invalid target url")]
    InvalidTargetUrl(#[from] tonic::codegen::http::uri::InvalidUri),
    #[error("Connection failed: {0}")]
    TlsError(#[from] tonic::transport::Error),
    #[error("IO error {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid job id argument")]
    InvalidJobId(#[from] common::InvalidJobIdString),
    #[error("Server returned invalid data (jobid bytes)")]
    InvalidJobIdBytes(#[from] common::InvalidJobIdBytes),
}

pub type DResult<T> = std::result::Result<T, Error>;
