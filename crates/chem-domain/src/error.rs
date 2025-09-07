use chemengine::EngineError;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    ExternalError(String),
}
impl From<EngineError> for DomainError {
    fn from(e: EngineError) -> Self {
        DomainError::ExternalError(e.to_string())
    }
}
