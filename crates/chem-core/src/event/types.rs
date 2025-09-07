//! Tipos de evento del flujo y estructura `FlowEvent`.
//!
//! Rol en el flujo:
//! - Cada ejecución del `FlowEngine` emite eventos a un `EventStore`
//!   append-only.
//! - Estos eventos permiten reconstruir el estado del `FlowRepository` (replay)
//!   sin depender de estructuras mutables.
//! - El enum `FlowEventKind` define el contrato observable y estable del motor.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::CoreEngineError;

/// Tipos de eventos soportados en F2 (esqueleto).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowEventKind {
    /// Emisión inicial de un flujo: fija la `definition_hash` y cantidad de
    /// steps. Invariante: Debe ser el primer evento de un `flow_id`.
    FlowInitialized { definition_hash: String, step_count: usize },
    /// Un step comenzó su ejecución. No implica éxito.
    StepStarted { step_index: usize, step_id: String },
    /// Un step terminó correctamente, con sus outputs (hashes) y fingerprint.
    StepFinished {
        step_index: usize,
        step_id: String,
        outputs: Vec<String>,
        fingerprint: String,
    },
    /// Un step terminó con error terminal. El flujo no continúa
    /// (stop-on-failure).
    StepFailed {
        step_index: usize,
        step_id: String,
        error: CoreEngineError,
        fingerprint: String,
    },
    /// Señal generada por el motor/step para comunicar un hito ligero (no
    /// altera estado principal).
    StepSignal {
        step_index: usize,
        step_id: String,
        signal: String,
        data: serde_json::Value,
    },
    /// Evento de cierre con fingerprint agregado del flow (hash de fingerprints
    /// ordenados de steps exitosos)
    FlowCompleted { flow_fingerprint: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEvent {
    pub seq: u64, // asignado por EventStore in-memory (orden append)
    pub flow_id: Uuid,
    pub kind: FlowEventKind,
    pub ts: DateTime<Utc>, // metadato (no entra en fingerprint)
}
