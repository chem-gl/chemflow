//! Evento y almacenamiento de eventos (Event Sourcing).
//!
//! Este módulo encapsula dos responsabilidades claramente separadas:
//! - `types`: definición del shape de eventos (`FlowEvent`, `FlowEventKind`).
//! - `store`: trait `EventStore` y una implementación en memoria para pruebas
//!   y desarrollo (`InMemoryEventStore`).
//!
//! Diseño:
//! - El `FlowEngine` solo depende del trait `EventStore` para escribir/listar
//!   eventos. Esto facilita sustituir la implementación por una persistente
//!   (Postgres, Dynamo, etc.) sin tocar la lógica del engine.
//!
//! Exportaciones públicas:
//! - `FlowEvent`, `FlowEventKind` (tipos de eventos).
//! - `EventStore`, `InMemoryEventStore` (contrato y una implementación).

mod store;
mod types;

pub use store::{EventStore, InMemoryEventStore};
pub use types::{FlowEvent, FlowEventKind};
