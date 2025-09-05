//! chem-core: Motor lineal determinista (F2) – Esqueleto
//! Sólo define contratos y estructuras básicas sin lógica implementada.

pub mod constants;
pub mod hashing;
pub mod model;
pub mod step;
pub mod event;
pub mod repo;
pub mod engine;
pub mod errors;

// Re-exports públicos principales
pub use engine::FlowEngine;
pub use model::{Artifact, ArtifactKind};
pub use step::{StepDefinition, StepKind, StepStatus, StepRunResult};
pub use event::{FlowEvent, FlowEventKind, EventStore, InMemoryEventStore};
pub use repo::{FlowRepository, build_flow_definition, InMemoryFlowRepository, FlowDefinition};
