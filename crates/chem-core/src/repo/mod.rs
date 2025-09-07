//! Repositorio del Flow (replay) y definiciones de Flow.
//!
//! Rol en el flujo:
//! - `FlowRepository` aplica la secuencia de `FlowEvent` para reconstruir el
//!   estado (`FlowInstance`). La implementación in-memory sirve como referencia
//!   simple y es reutilizada por el backend Postgres.
//! - `FlowDefinition` captura los steps en orden y su `definition_hash`.
pub mod types;
pub use types::{build_flow_definition, build_flow_definition_auto, FlowDefinition, InMemoryFlowRepository};
pub use types::{FlowInstance, FlowRepository, StepSlot};
