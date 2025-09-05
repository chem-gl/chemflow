//! Definiciones de eventos y trait EventStore.

mod types;
mod store;

pub use types::{FlowEvent, FlowEventKind};
pub use store::EventStore;
pub use store::InMemoryEventStore;