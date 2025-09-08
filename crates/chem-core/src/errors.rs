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
