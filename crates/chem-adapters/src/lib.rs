//! chem-adapters: Capa de adaptación Dominio ↔ Core (F4)
//!
//! Este crate provee:
//! - Artifacts tipados neutrales (sin semántica en el core).
//! - Un trait `DomainArtifactEncoder` para empaquetar tipos de dominio en `Artifact`.
//! - Steps iniciales: `AcquireMoleculesStep` (Source determinista) y
//!   `ComputePropertiesStep` (Transform stub) para validar el pipeline
//!   Acquire→Compute.
//!
//! Nota: El core sólo conoce `Artifact { kind, hash, payload, metadata }`
//! y `ArtifactKind::GenericJson`. Aquí nos apoyamos en artifacts tipados que
//! serializan a payload JSON y en los macros del core para Steps tipados.

pub mod artifacts;
pub mod encoder;
pub mod steps;
pub mod injectors;

pub use injectors::FamilyHashInjector;
pub use injectors::PropertiesInjector;

use chem_core::{EventStore, FlowRepository, FlowEngine};

/// Helper: construct a FlowEngine with the adapters' injectors pre-registered.
pub fn new_engine_with_adapters<E: EventStore, R: FlowRepository>(event_store: E, repository: R) -> FlowEngine<E, R> {
	let mut engine = FlowEngine::new_with_stores(event_store, repository);
	engine.injectors.push(Box::new(FamilyHashInjector));
	engine.injectors.push(Box::new(PropertiesInjector));
	engine
}


