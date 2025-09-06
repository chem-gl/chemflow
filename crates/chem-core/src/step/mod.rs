//! Definiciones relacionadas a Steps.

pub mod definition;
pub mod typed;
pub mod pipeline;
pub mod macros; // macros for typed artifacts and steps
mod status;
mod run_result;

pub use definition::{StepDefinition, StepKind};
pub use typed::{TypedStep, StepRunResultTyped};
pub use pipeline::{Pipe, SameAs};
pub use status::StepStatus;
pub use run_result::{StepRunResult, StepSignal};
