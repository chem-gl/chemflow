use crate::{errors::CoreEngineError, model::Artifact};
use serde_json::Value;

/// Señal emitida opcionalmente por un Step sin afectar el flujo principal.
#[derive(Debug, Clone)]
pub struct StepSignal { pub signal: String, pub data: Value }

/// Resultado abstracto de ejecutar un step.
pub enum StepRunResult {
    Success { outputs: Vec<Artifact> },
    /// Igual que Success pero con señales (metadatos livianos) que el engine traducirá a eventos.
    SuccessWithSignals { outputs: Vec<Artifact>, signals: Vec<StepSignal> },
    Failure { error: CoreEngineError },
}
