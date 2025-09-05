use crate::{errors::CoreEngineError, model::Artifact};

/// Resultado abstracto de ejecutar un step.
pub enum StepRunResult {
    Success { outputs: Vec<Artifact> },
    Failure { error: CoreEngineError },
}
