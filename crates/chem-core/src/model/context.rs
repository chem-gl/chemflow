use serde_json::Value;

use super::Artifact;

/// Contexto de ejecución entregado al StepDefinition::run
pub struct ExecutionContext {
    pub input: Option<Artifact>, // Artifact único encadenado (None primer step)
    pub params: Value,           // parámetros canónicos
}
