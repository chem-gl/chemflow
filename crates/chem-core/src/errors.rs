//! Errores específicos del core (simples por ahora).

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum CoreEngineError {
    #[error("flow already completed")] FlowCompleted,
    #[error("invalid step index")] InvalidStepIndex,
    #[error("step already terminal")] StepAlreadyTerminal,
    #[error("missing required inputs")] MissingInputs,
    #[error("internal: {0}")] Internal(String),
}
