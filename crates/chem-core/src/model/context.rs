use serde_json::Value;

use super::Artifact;

/// Contexto de ejecución entregado al StepDefinition::run
pub struct ExecutionContext {
    pub inputs: Vec<Artifact>, // outputs previos filtrados / requeridos
    pub params: Value,         // parámetros canónicos
}
