//! Tipos de repositorio: estado reconstruido (FlowInstance) y definición
//! (FlowDefinition).
//!
//! En F2, el repositorio aplica un replay lineal: consume eventos en orden y
//! actualiza un `FlowInstance` inmutable por evento. No almacena artifacts
//! completos (sólo hashes) para mantener neutralidad.
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::event::{FlowEvent, FlowEventKind};
use crate::step::{StepDefinition, StepStatus};

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
    pub attempts: u32, // (Futuro retries) inicial primera ejecución =1
    /// Conteo acumulado de reintentos agendados/consumidos (Failed→Pending transiciones).
    pub retry_count: u32,
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
    pub fn new(steps: Vec<Box<dyn StepDefinition>>, definition_hash: String) -> Self {
        Self { steps, definition_hash }
    }
    pub fn len(&self) -> usize {
        self.steps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

pub struct InMemoryFlowRepository;
impl InMemoryFlowRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InMemoryFlowRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowRepository for InMemoryFlowRepository {
    fn load(&self, flow_id: Uuid, events: &[FlowEvent], definition: &FlowDefinition) -> FlowInstance {
        let mut steps: Vec<StepSlot> = definition.steps
                                                 .iter()
                                                 .map(|s| StepSlot { step_id: s.id().to_string(),
                                                                     status: StepStatus::Pending,
                                                                     fingerprint: None,
                                                                     outputs: vec![],
                                                                     started_at: None,
                                                                     finished_at: None,
                                                                     attempts: 0,
                                                                     retry_count: 0 })
                                                 .collect();
        let mut completed = false;
        for ev in events {
            match &ev.kind {
                FlowEventKind::FlowInitialized { .. } => {}
                FlowEventKind::StepStarted { step_index, .. } => {
                    if let Some(slot) = steps.get_mut(*step_index) {
                        slot.status = StepStatus::Running;
                        slot.started_at = Some(ev.ts);
                        slot.attempts += 1;
                    }
                }
                FlowEventKind::StepFinished { step_index,
                                              fingerprint,
                                              outputs,
                                              .. } => {
                    if let Some(slot) = steps.get_mut(*step_index) {
                        slot.status = StepStatus::FinishedOk;
                        slot.fingerprint = Some(fingerprint.clone());
                        slot.outputs = outputs.clone();
                        slot.finished_at = Some(ev.ts);
                    }
                }
                FlowEventKind::StepFailed { step_index, fingerprint, .. } => {
                    if let Some(slot) = steps.get_mut(*step_index) {
                        slot.status = StepStatus::Failed;
                        slot.fingerprint = Some(fingerprint.clone());
                        slot.finished_at = Some(ev.ts);
                    }
                }
                // F7: Al rehidratar, aplicar la transición Failed → Pending si
                // corresponde y aumentar retry_count. El índice del step se
                // infiere buscando el slot por step_id.
                FlowEventKind::RetryScheduled { step_id, retry_index, .. } => {
                    if let Some((idx, slot)) = steps.iter_mut().enumerate().find(|(_, s)| &s.step_id == step_id) {
                        // Sólo si estaba Failed y el retry_index es consistente (retry_count+1)
                        if matches!(slot.status, StepStatus::Failed) {
                            let expected = slot.retry_count + 1;
                            if *retry_index == expected {
                                slot.retry_count = *retry_index;
                                slot.status = StepStatus::Pending;
                                // Reposicionar cursor si es anterior al índice del step a reintentar
                                // El cursor se define como el primer Pending; recalcularemos al final.
                            } else {
                                // Si el índice no es consistente, ignoramos el evento para mantener invariantes.
                                let _ = idx; // hint to avoid unused warning
                            }
                        }
                    }
                }
                FlowEventKind::FlowCompleted { .. } => completed = true,
                FlowEventKind::StepSignal { .. } => {}
                FlowEventKind::PropertyPreferenceAssigned { .. } => {}
            }
        }
        // Cursor: primer Pending; si no hay, posición = len(). Esto soporta
        // reintentos: un RetryScheduled marca Pending el step Failed, por lo que
        // el cursor vuelve a ese índice.
        let cursor = steps.iter()
                          .position(|s| matches!(s.status, StepStatus::Pending))
                          .unwrap_or(steps.len());
        FlowInstance { id: flow_id,
                       steps,
                       cursor,
                       completed }
    }
}

pub fn build_flow_definition(step_ids: &[&str], steps: Vec<Box<dyn StepDefinition>>) -> FlowDefinition {
    use crate::hashing::{hash_str, to_canonical_json};
    use serde_json::json;
    let ids_json = json!(step_ids);
    let canonical = to_canonical_json(&ids_json);
    let definition_hash = hash_str(&canonical);
    FlowDefinition::new(steps, definition_hash)
}

/// Builder alternativo: recibe directamente los steps y extrae sus ids en
/// orden. Facilita al usuario no tener que mantener manualmente el arreglo
/// `step_ids`.
pub fn build_flow_definition_auto(steps: Vec<Box<dyn StepDefinition>>) -> FlowDefinition {
    let ids: Vec<String> = steps.iter().map(|s| s.id().to_string()).collect();
    let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
    build_flow_definition(&id_refs, steps)
}
