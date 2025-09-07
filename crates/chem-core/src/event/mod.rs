//! Evento y almacenamiento de eventos.
//!
//! Este módulo agrupa:
//! - `types`: enum `FlowEventKind` y struct `FlowEvent` que representan los
//!   hechos del flujo en un log inmutable.
//! - `store`: trait `EventStore` y la implementación `InMemoryEventStore`.
//!
//! El `FlowEngine` sólo depende del trait, permitiendo cambiar el backend
//! (memoria, Postgres, etc.) sin afectar la lógica.

mod store;
mod types;

pub use store::EventStore;
pub use store::InMemoryEventStore;
pub use types::{FlowEvent, FlowEventKind};
