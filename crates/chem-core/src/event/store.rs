use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;

use super::{FlowEvent, FlowEventKind};

/// Almacenamiento de eventos append-only.
pub trait EventStore {
    /// Agrega un evento a partir de su kind y devuelve el evento completo (con seq y ts).
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent;
    /// Lista eventos de un flujo (orden ascendente por seq).
    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent>;
}


pub struct InMemoryEventStore { pub inner: HashMap<Uuid, Vec<FlowEvent>> }

impl Default for InMemoryEventStore { fn default() -> Self { Self { inner: HashMap::new() } } }

impl EventStore for InMemoryEventStore {
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent {
        let vec = self.inner.entry(flow_id).or_insert_with(Vec::new);
        let seq = vec.len() as u64;
        let ev = FlowEvent { seq, flow_id, kind, ts: Utc::now() };
        vec.push(ev.clone());
        ev
    }
    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent> { self.inner.get(&flow_id).cloned().unwrap_or_default() }
}
