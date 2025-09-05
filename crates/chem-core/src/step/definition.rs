use serde_json::Value;

use crate::model::ExecutionContext;
use super::run_result::StepRunResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind { Source, Transform, Sink, Check }

/// Trait que define un Step. Implementaciones deben ser puras respecto a inputs + params.
pub trait StepDefinition {
    /// Identificador estable y único dentro del Flow.
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
