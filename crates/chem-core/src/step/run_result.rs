//! Resultado de la ejecución de un Step y tipos auxiliares.
//!
//! El engine traduce estos resultados a eventos y gestiona el estado del
//! flujo. `StepSignal` son anotaciones ligeras emitidas por un step y que
//! pueden transformarse en eventos por el engine sin afectar la máquina de
//! estados principal.
//!
//! Tipos:
//! - `StepSignal`: señal ligera con nombre y payload JSON.
//! - `StepRunResult`: resultado neutral usado por `StepDefinition::run`.

use crate::{errors::CoreEngineError, model::Artifact};
use serde_json::Value;

/// Señal emitida opcionalmente por un Step sin afectar el flujo principal.
#[derive(Debug, Clone)]
pub struct StepSignal {
    /// Nombre de la señal (p. ej. "progress", "warning").
    pub signal: String,
    /// Payload arbitrario en JSON.
    pub data: Value,
}

/// Resultado neutral de ejecutar un Step.
///
/// Variantes:
/// - `Success` contiene los artifacts resultantes.
/// - `SuccessWithSignals` incluye además señales que el engine podrá emitir
///   como eventos auxiliares.
/// - `Failure` incorpora un `CoreEngineError` con información del fallo.
pub enum StepRunResult {
    Success { outputs: Vec<Artifact> },
    SuccessWithSignals { outputs: Vec<Artifact>, signals: Vec<StepSignal> },
    Failure { error: CoreEngineError },
}
