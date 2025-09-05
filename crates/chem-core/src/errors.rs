//! Errores específicos del core (simples por ahora).

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum CoreEngineError {
    #[error("flow already completed")] FlowCompleted,
    #[error("invalid step index")] InvalidStepIndex,
    #[error("step already terminal")] StepAlreadyTerminal,
    #[error("missing required inputs")] MissingInputs,
    #[error("first step must be source kind")] FirstStepMustBeSource,
    #[error("flow has failed previously (stop-on-failure invariant)")] FlowHasFailed,
    #[error("internal: {0}")] Internal(String),
}
