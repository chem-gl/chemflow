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
    /// F6: Evento de política de selección que asigna una preferencia de
    /// propiedad. No altera la máquina de estados ni el orden del flujo; es
    /// auditable y determinista. Debe emitirse antes del StepFinished que lo
    /// origina.
    PropertyPreferenceAssigned {
        /// Clave estable de la propiedad seleccionada (p.ej., inchikey+kind o id lógico).
        property_key: String,
        /// Identificador estable de la política de selección (p.ej., "max_score").
        policy_id: String,
        /// Hash canónico de los parámetros de la política (influye en fingerprint del step via params del step).
        params_hash: String,
        /// Rationale serializado en JSON canónico (además puede existir forma tipada en capas superiores).
        rationale: serde_json::Value,
    },
    /// Evento que representa la creación de una rama (branch) a partir de un
    /// `parent_flow` y un step concreto. Contiene un `divergence_params_hash`
    /// que resume los parámetros que divergen en la nueva rama.
    BranchCreated {
        branch_id: Uuid,
        parent_flow_id: Uuid,
        root_flow_id: Uuid,
        created_from_step_id: String,
        divergence_params_hash: Option<String>,
    },
    /// F7: Evento que agenda un reintento manual para un `step_id` que está en
    /// estado Failed. No altera el fingerprint ni introduce efectos laterales
    /// por sí mismo; su efecto se aplica en el replay del repositorio
    /// (Failed → Pending), permitiendo una nueva ejecución del step.
    ///
    /// Invariantes:
    /// - Debe emitirse únicamente si el step está Failed.
    /// - `retry_index` cuenta desde 1 (primer reintento) y es determinista en
    ///   base al conteo de `StepStarted` ocurridos tras el primer `StepFailed`.
    /// - `reason` es opcional y no participa de ningún hash/fingerprint.
    RetryScheduled {
        /// Identificador lógico del step dentro de la definición.
        step_id: String,
        /// Índice de reintento (1-based) programado.
        retry_index: u32,
        /// Razón humana opcional (no canónica; sólo informativa).
        reason: Option<String>,
    },
    /// Evento que indica que el engine solicita input humano para continuar.
    /// Contiene un `schema` opcional para validar el input y `hint` descriptivo.
    UserInteractionRequested {
        step_index: usize,
        step_id: String,
        schema: Option<serde_json::Value>,
        hint: Option<String>,
    },
    /// Evento que representa que el usuario/proveedor externo proveyó input.
    /// `decision_hash` es el hash canónico del rationale/overrides que sí
    /// participa (si corresponde) en el fingerprint si fue marcado como override.
    UserInteractionProvided {
        step_index: usize,
        step_id: String,
        provided: serde_json::Value,
        decision_hash: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEvent {
    pub seq: u64, // asignado por EventStore in-memory (orden append)
    pub flow_id: Uuid,
    pub kind: FlowEventKind,
    pub ts: DateTime<Utc>, // metadato (no entra en fingerprint)
}
