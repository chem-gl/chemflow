use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::CoreEngineError;

/// Tipos de eventos soportados en F2 (esqueleto).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowEventKind {
    FlowInitialized { definition_hash: String, step_count: usize },
    StepStarted { step_index: usize, step_id: String },
    StepFinished { step_index: usize, step_id: String, outputs: Vec<String>, fingerprint: String },
    StepFailed { step_index: usize, step_id: String, error: CoreEngineError, fingerprint: String },
    FlowCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEvent {
    pub seq: u64,             // asignado por EventStore in-memory (orden append)
    pub flow_id: Uuid,
    pub kind: FlowEventKind,
    pub ts: DateTime<Utc>,    // metadato (no entra en fingerprint)
}
