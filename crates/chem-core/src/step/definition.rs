use serde_json::Value;

use crate::model::ExecutionContext;
use super::run_result::StepRunResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind { Source, Transform, Sink, Check }

 pub trait StepDefinition {
     fn id(&self) -> &str;

    /// Nombre opcional amigable.
    fn name(&self) -> &str { self.id() }

    /// Parámetros base deterministas (defaults). Se fusionarán con overrides futuros.
    fn base_params(&self) -> Value;

    /// Ejecución pura del step. Debe usar únicamente inputs + params.
    fn run(&self, ctx: &ExecutionContext) -> StepRunResult;

    /// Tipo general del step.
    fn kind(&self) -> StepKind;
}
