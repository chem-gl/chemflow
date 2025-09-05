//! Errores de persistencia.
//! Mapea errores de Diesel / conexión a variantes semánticas del dominio de persistencia.

use thiserror::Error;
use diesel::result::{DatabaseErrorKind, Error as DieselError};

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("unique violation: {0}")] 
    UniqueViolation(String),
    #[error("check violation: {0}")] 
    CheckViolation(String),
    #[error("foreign key violation: {0}")] 
    ForeignKeyViolation(String),
    #[error("not found")] 
    NotFound,
    #[error("serialization conflict (retryable)")] 
    SerializationConflict,
    #[error("transient IO / connection pool error: {0}")] 
    TransientIo(String),
    #[error("unknown database error: {0}")] 
    Unknown(String),
}

impl From<DieselError> for PersistenceError {
    fn from(err: DieselError) -> Self {
        match err {
            DieselError::NotFound => Self::NotFound,
            DieselError::DatabaseError(kind, info) => match kind {
                        DatabaseErrorKind::UniqueViolation => Self::UniqueViolation(info.message().to_string()),
                        DatabaseErrorKind::CheckViolation => Self::CheckViolation(info.message().to_string()),
                        DatabaseErrorKind::ForeignKeyViolation => Self::ForeignKeyViolation(info.message().to_string()),
                        DatabaseErrorKind::SerializationFailure => Self::SerializationConflict,
                        other => Self::Unknown(format!("db error kind {:?}: {}", other, info.message())),
                    },
            DieselError::DeserializationError(e) => Self::Unknown(format!("deser: {e}")),
            DieselError::SerializationError(e) => Self::Unknown(format!("ser: {e}")),
            DieselError::AlreadyInTransaction => Self::Unknown("already in transaction".into()),
            DieselError::RollbackErrorOnCommit { rollback_error, commit_error } => Self::Unknown(format!("rollback={rollback_error}; commit={commit_error}")),
            DieselError::BrokenTransactionManager => Self::TransientIo("broken transaction manager".into()),
            DieselError::QueryBuilderError(e) => Self::Unknown(format!("query builder: {e}")),
            DieselError::InvalidCString(e) => Self::Unknown(format!("invalid cstring: {e}")),
            DieselError::RollbackTransaction => Self::Unknown("rollback transaction".into()),
            DieselError::NotInTransaction => Self::Unknown("not in transaction".into()),
            other => Self::Unknown(format!("unhandled diesel error: {other:?}")),
        }
    }
}
