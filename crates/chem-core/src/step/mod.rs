//! Definiciones relacionadas a Steps.
//!
//! En el flujo F2, un Step es una unidad determinista que transforma a lo sumo
//! un `Artifact` de entrada en 0..n artifacts de salida. Este módulo define:
//! - `StepDefinition`: interfaz neutral usada por el engine.
//! - `TypedStep`: interfaz de alto nivel (opcional) con tipos fuertes.
//! - `StepRunResult` y señales (`StepSignal`).
//! - `Pipe` para construir pipelines tipados que validan IO en compilación.

pub mod definition;
pub mod macros; // macros for typed artifacts and steps
pub mod pipeline;
mod run_result;
mod status;
pub mod typed;

pub use definition::{StepDefinition, StepKind};
pub use pipeline::{Pipe, SameAs};
pub use run_result::{StepRunResult, StepSignal};
pub use status::StepStatus;
pub use typed::{StepRunResultTyped, TypedStep};
