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
    #[error("internal: {0}")]
    Internal(String),
}
