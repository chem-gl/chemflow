//! Módulo: almacenamiento de eventos (event sourcing) para "flows".
//!
//! Este módulo define una interfaz (trait) para un almacén de eventos
//! append-only y una implementación en memoria. La idea es que cada "flow"
//! (identificado por un UUID) acumula una secuencia ordenada de eventos
//! (FlowEvent). Cada evento tiene:
//! - seq: número de secuencia creciente (0, 1, 2, ...)
//! - flow_id: identificador del flujo al que pertenece
//! - kind: el "tipo" o payload del evento (FlowEventKind)
//! - ts: marca de tiempo en UTC en el momento del append
//!
//! El almacén es "append-only": solo se agregan eventos al final. No hay
//! mutaciones ni borrados de eventos existentes. Esto facilita reproducir
//! el estado reconstruyendo desde el log de eventos.

use chrono::Utc; // Fuente de tiempo en UTC para timestamp de eventos.
use std::collections::HashMap;
use uuid::Uuid; // Identificador único para cada "flow".

use super::{FlowEvent, FlowEventKind};

/// Almacenamiento de eventos append-only para "flows".
///
/// Contrato principal:
/// - `append_kind` añade un evento determinista al final del log de `flow_id`
///   y asigna `seq` y `ts`.
/// - `list` devuelve todos los eventos del `flow_id` en orden ascendente por
///   `seq`.
///
/// La intención es mantener esta interfaz mínima y fácil de implementar por
/// backends distintos (memoria, bases de datos, colas con orden garantizado,
/// etc.).
pub trait EventStore {
    /// Agrega un evento a partir de su `kind` y devuelve el `FlowEvent`
    /// persistido (incluye `seq` y `ts`).
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent;

    /// Lista eventos de un flujo en orden ascendente por `seq`.
    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent>;
}

/// Implementación en memoria del `EventStore`.
///
/// - Volátil: los datos se pierden al finalizar el proceso.
/// - No es sincronizada por hilos: si se necesita concurrencia envolver en
///   `Mutex`/`RwLock` o proporcionar una variante thread-safe.
/// - Útil para tests y para ejecutar el engine en memoria.
pub struct InMemoryEventStore {
    pub inner: HashMap<Uuid, Vec<FlowEvent>>,
}

impl InMemoryEventStore {
    /// Crea un store vacío.
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }

    /// Helper: obtiene el número de eventos actualmente almacenados para un flow.
    pub fn len_for(&self, flow_id: Uuid) -> usize {
        self.inner.get(&flow_id).map(|v| v.len()).unwrap_or(0)
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent {
        let vec = self.inner.entry(flow_id).or_default();

        // Sequence number is 0-based and equal to current length of the vector.
        let seq = vec.len() as u64;

        let ev = FlowEvent { seq, flow_id, kind, ts: Utc::now() };

        // Push and return a clone to keep ownership semantics clear.
        vec.push(ev.clone());
        ev
    }

    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent> {
        self.inner.get(&flow_id).cloned().unwrap_or_default()
    }
}
