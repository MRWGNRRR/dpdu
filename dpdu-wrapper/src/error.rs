pub type GeneralResult<T> = ::std::result::Result<T, GeneralError>;

#[derive(Debug, Clone, thiserror::Error)]
#[error("general pdu error: {0}")]
pub enum GeneralError {
    #[error("api error: {0}")]
    ApiError(#[from] crate::api::ApiError),

    #[error("worker error: {0}")]
    WorkerError(#[from] crate::worker::WorkerError),

    #[error("primitive error: {0}")]
    PrimitiveError(#[from] crate::types::pdu_com_primitive::PrimitiveError)
}