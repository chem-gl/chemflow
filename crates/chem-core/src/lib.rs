//! chem-core: Motor lineal determinista (F2)
//!
//! Propósito:
//! - Proveer los contratos neutrales (sin semántica química) y la orquestación
//!   mínima para ejecutar un flujo lineal de Steps de manera determinista.
//! - Generar una secuencia de eventos (Event Sourcing) reproducible y capaz de
//!   reconstruir el estado (replay) sin mutar datos históricos.
//!
//! Componentes principales:
//! - `step`: definición de Steps (neutrales y tipados) y resultados de
//!   ejecución.
//! - `event`: tipos de eventos del flujo y trait `EventStore` (in-memory +
//!   backends).
//! - `repo`: reconstrucción (`FlowRepository`) del estado a partir de eventos.
//! - `engine`: orquestador `FlowEngine` que aplica la definición paso a paso.
//! - `model`: tipos neutrales como `Artifact` y utilidades de tipado fuerte
//!   opcional.
//! - `hashing`: canonicalización JSON y helpers de hash para fingerprints.
//! - `errors`: errores semánticos del engine.
//!
//! Re-exports: se exponen símbolos clave para facilitar el uso desde
//! binarios/tests.

pub mod constants;
pub mod engine;
pub mod errors;
pub mod event;
pub mod hashing;
pub mod model;
pub mod repo;
pub mod step;
pub mod injection;

// Re-exports públicos principales
pub use engine::{FlowCtx, FlowEngine};
pub use event::{EventStore, FlowEvent, FlowEventKind, InMemoryEventStore};
pub use model::{Artifact, ArtifactKind};
pub use repo::{build_flow_definition, FlowDefinition, FlowRepository, InMemoryFlowRepository};
pub use step::{Pipe, SameAs, StepDefinition, StepKind, StepRunResult, StepRunResultTyped, StepStatus, TypedStep};
// Re-export injection API for external users/tests
pub use injection::{CompositeInjector, ParamInjector};
