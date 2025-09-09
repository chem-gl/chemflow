//! Errores específicos del core.
//!
//! Estos errores modelan condiciones terminales o inválidas del motor, por
//! ejemplo intentar avanzar un flujo ya completado o sin inputs requeridos.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum CoreEngineError {
    #[error("flow already completed")]
    FlowCompleted,
    #[error("invalid step index")]
    InvalidStepIndex,
    #[error("step already terminal")]
    StepAlreadyTerminal,
    #[error("missing required inputs")]
    MissingInputs,
    #[error("first step must be source kind")]
    FirstStepMustBeSource,
    #[error("flow has failed previously (stop-on-failure invariant)")]
    FlowHasFailed,
    #[error("invalid branch source: step must be FinishedOk")]
    InvalidBranchSource,
    // F7 – Errores de política/estado para reintentos
    #[error("retry not allowed for step '{step_id}': {reason}")]
    RetryNotAllowed { step_id: String, reason: String },
    #[error("invalid transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },
    #[error("policy violation: {0}")]
    PolicyViolation(String),
    #[error("storage error: {0}")]
    StorageError(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// Clasificación de errores para persistencia extendida (F8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    Transient,
    Permanent,
    Validation,
    Runtime,
}

/// Clasifica un error del engine en una categoría estable para políticas de retry y auditoría.
pub fn classify_error(error: &CoreEngineError) -> ErrorClass {
    match error {
        CoreEngineError::Internal(_) | CoreEngineError::StorageError(_) => ErrorClass::Runtime,
    CoreEngineError::InvalidStepIndex | CoreEngineError::MissingInputs | CoreEngineError::FirstStepMustBeSource | CoreEngineError::StepAlreadyTerminal | CoreEngineError::FlowCompleted | CoreEngineError::FlowHasFailed | CoreEngineError::InvalidBranchSource | CoreEngineError::RetryNotAllowed { .. } | CoreEngineError::InvalidTransition { .. } | CoreEngineError::PolicyViolation(_) => ErrorClass::Validation,
    }
}
