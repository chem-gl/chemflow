use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::step::{StepStatus, StepDefinition};
use crate::event::{FlowEvent, FlowEventKind};

 pub struct FlowInstance {
    pub id: Uuid,
    pub steps: Vec<StepSlot>,
    pub cursor: usize,
    pub completed: bool,
}

/// Estado de un step en la instancia.
pub struct StepSlot {
    pub step_id: String,
    pub status: StepStatus,
    pub fingerprint: Option<String>,
    pub outputs: Vec<String>, // almacenar solo hashes aquí (Artifacts completos podrían gestionarse aparte)
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// Trait para reconstruir (`replay`) estado de un flow a partir de eventos.
pub trait FlowRepository {
    fn load(&self, flow_id: Uuid, events: &[FlowEvent], definition: &FlowDefinition) -> FlowInstance;
}

/// Definición inmutable del Flow.
pub struct FlowDefinition {
    pub steps: Vec<Box<dyn StepDefinition>>,
    pub definition_hash: String,
}

impl FlowDefinition {
    pub fn new(steps: Vec<Box<dyn StepDefinition>>, definition_hash: String) -> Self { Self { steps, definition_hash } }
    pub fn len(&self) -> usize { self.steps.len() }
}

pub struct InMemoryFlowRepository;
impl InMemoryFlowRepository { pub fn new() -> Self { Self } }

impl FlowRepository for InMemoryFlowRepository {
    fn load(&self, flow_id: Uuid, events: &[FlowEvent], definition: &FlowDefinition) -> FlowInstance {
        let mut steps: Vec<StepSlot> = definition.steps.iter().map(|s| StepSlot {
            step_id: s.id().to_string(),
            status: StepStatus::Pending,
            fingerprint: None,
            outputs: vec![],
            started_at: None,
            finished_at: None,
        }).collect();
        let mut completed = false;
        for ev in events {
            match &ev.kind {
                FlowEventKind::FlowInitialized { .. } => {},
                FlowEventKind::StepStarted { step_index, .. } => {
                                if let Some(slot) = steps.get_mut(*step_index) { slot.status = StepStatus::Running; slot.started_at = Some(ev.ts); }
                            }
                FlowEventKind::StepFinished { step_index, fingerprint, outputs, .. } => {
                                if let Some(slot) = steps.get_mut(*step_index) { slot.status = StepStatus::FinishedOk; slot.fingerprint = Some(fingerprint.clone()); slot.outputs = outputs.clone(); slot.finished_at = Some(ev.ts); }
                            }
                FlowEventKind::StepFailed { step_index, fingerprint, .. } => {
                                if let Some(slot) = steps.get_mut(*step_index) { slot.status = StepStatus::Failed; slot.fingerprint = Some(fingerprint.clone()); slot.finished_at = Some(ev.ts); }
                            }
                FlowEventKind::FlowCompleted => completed = true,
                FlowEventKind::StepSignal { .. } => { /* no-op: señales no alteran estado central */ },
            }
        }
        let cursor = steps.iter().position(|s| matches!(s.status, StepStatus::Pending)).unwrap_or(steps.len());
        FlowInstance { id: flow_id, steps, cursor, completed }
    }
}

pub fn build_flow_definition(step_ids: &[&str], steps: Vec<Box<dyn StepDefinition>>) -> FlowDefinition {
    use crate::hashing::{to_canonical_json, hash_str};
    use serde_json::json;
    let ids_json = json!(step_ids);
    let canonical = to_canonical_json(&ids_json);
    let definition_hash = hash_str(&canonical);
    FlowDefinition::new(steps, definition_hash)
}

