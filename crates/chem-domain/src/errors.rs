// errors.rs
use chemengine::EngineError;
use thiserror::Error;

/// Error personalizado del dominio para la aplicación química
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Error de validación: {0}")]
    ValidationError(String),

    #[error("Error externo: {0}")]
    ExternalError(String),

    #[error("Error de serialización: {0}")]
    SerializationError(String),
}

// Implementación de conversión desde EngineError a DomainError
impl From<EngineError> for DomainError {
    fn from(e: EngineError) -> Self {
        DomainError::ExternalError(e.to_string())
    }
}

// Implementación de conversión desde serde_json::Error a DomainError
impl From<serde_json::Error> for DomainError {
    fn from(e: serde_json::Error) -> Self {
        DomainError::SerializationError(e.to_string())
    }
}
