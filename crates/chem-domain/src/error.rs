use thiserror::Error;
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("{0}")]
    ValidationError(String),
    #[error("{0}")]
    ExternalError(String),
}
