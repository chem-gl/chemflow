use serde_json::Value;

use crate::model::ArtifactKind;
use crate::model::ExecutionContext;
use super::run_result::StepRunResult;

/// Clasificación neutral de steps (no semántica química)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind { Source, Transform, Sink, Check }

/// Trait que define un Step. Implementaciones deben ser puras respecto a inputs + params.
pub trait StepDefinition {
    /// Identificador estable y único dentro del Flow.
    fn id(&self) -> &str;

    /// Nombre opcional amigable.
    fn name(&self) -> &str { self.id() }

    /// Tipos de artifacts requeridos como input. Orden no relevante.
    fn required_input_kinds(&self) -> &[ArtifactKind];

    /// Parámetros base deterministas (defaults). Se fusionarán con overrides futuros.
    fn base_params(&self) -> Value;

    /// Ejecución pura del step. Debe usar únicamente inputs + params.
    /// TODO: Implementaciones concretas en crates adaptadores.
    fn run(&self, ctx: &ExecutionContext) -> StepRunResult;

    /// Tipo general del step.
    fn kind(&self) -> StepKind;
}
