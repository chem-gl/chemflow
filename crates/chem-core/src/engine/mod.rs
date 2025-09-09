//! FlowEngine – punto de orquestación del flujo F2.
//!
//! Responsabilidades principales:
//! - Cargar o inicializar el flujo (`FlowInitialized`).
//! - Avanzar step a step emitiendo `StepStarted`, `StepFinished`/`StepFailed` y
//!   señales (`StepSignal`) según corresponda.
//! - Al completar todos los steps exitosamente, emitir `FlowCompleted` con el
//!   fingerprint agregado.
//! - Mantener un store de artifacts en memoria para wiring de outputs entre
//!   steps.
//!
//! Invariantes clave:
//! - Append-only: el `EventStore` sólo agrega eventos al final por `flow_id`.
//! - Stop-on-failure: si un step falla, el flujo no continúa.
//! - Determinismo: fingerprint depende sólo de (engine_version,
//!   definition_hash, input_hashes ordenados y params canonicalizados).

use std::collections::HashMap;
use uuid::Uuid;

use crate::constants::ENGINE_VERSION;
use crate::errors::CoreEngineError;
use crate::event::{EventStore, FlowEventKind};
use crate::hashing::{hash_str, to_canonical_json};
use crate::model::{Artifact, ArtifactSpec, ExecutionContext, StepFingerprintInput, TypedArtifact};
use crate::repo::{FlowDefinition, FlowInstance, FlowRepository};
use crate::step::{StepRunResult, StepSignal, StepStatus};
// F7: Tipos simples de política de retry (deterministas, fuera de fingerprint)
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffKind,
}
#[derive(Clone, Debug)]
pub enum BackoffKind {
    None,
    Exponential { base_ms: u64 },
}
impl RetryPolicy {
    pub fn should_retry(&self, retry_count: u32) -> bool { retry_count < self.max_retries }
    /// Backoff determinista (sólo cálculo; no sleep). No afecta fingerprint.
    pub fn next_delay_ms(&self, retry_index: u32) -> u64 {
        match self.backoff {
            BackoffKind::None => 0,
            BackoffKind::Exponential { base_ms } => base_ms.saturating_mul(1u64 << (retry_index.saturating_sub(1) as u32)),
        }
    }
}

/// Estado interno de ejecución de un step antes de serializar a eventos.
struct ExecutionOutcome {
    fingerprint: String,
    output_hashes: Vec<String>,
    signals: Vec<StepSignal>,
    status: ExecutionStatus,
}

enum ExecutionStatus {
    Success,
    Failure(CoreEngineError),
}

/// Motor lineal determinista (F2). Mantiene referencias a contratos de
/// almacenamiento.
pub struct FlowEngine<E: EventStore, R: FlowRepository> {
    pub event_store: E,
    pub repository: R,
    pub artifact_store: HashMap<String, Artifact>,
    /// Definición por defecto opcional para simplificar el uso del engine
    /// sin tener que pasar `definition` en cada llamada.
    default_definition: Option<FlowDefinition>,
    /// Identificador del flow por defecto (opcional). Si está configurado,
    /// los métodos `*_default_flow` no requieren `flow_id` explícito.
    default_flow_id: Option<uuid::Uuid>,
    /// Nombre descriptivo opcional del flow por defecto (sólo informativo).
    default_flow_name: Option<String>,
    // Métricas internas F7 (no persistentes):
    retries_scheduled: u64,
    retries_rejected: u64,
}

/// Wrapper ergonómico que fija `flow_id` y `definition` para evitar repetir
/// parámetros.
pub struct FlowCtx<'a, E: EventStore, R: FlowRepository> {
    engine: &'a mut FlowEngine<E, R>,
    flow_id: Uuid,
    definition: &'a FlowDefinition,
}

impl<E: EventStore, R: FlowRepository> FlowEngine<E, R> {
    /// Alias ergonómico para construir el builder pasando los stores
    /// explícitos.
    /// Uso:
    ///   FlowEngine::builder(event_store,
    /// repo).firstStep(...).add_step(...).build()
    pub fn builder(event_store: E, repository: R) -> EngineBuilderInit<E, R> {
        EngineBuilderInit { event_store, repository }
    }

    /// Crea un nuevo motor (genérico) sin definición por defecto.
    /// Nota: en el modo in-memory existe una API ergonómica
    /// `FlowEngine::new(definition)`. Para la versión genérica use
    /// `new_with_stores`.
    pub fn new_with_stores(event_store: E, repository: R) -> Self {
        Self { event_store,
               repository,
               artifact_store: HashMap::new(),
               default_definition: None,
               default_flow_id: None,
               default_flow_name: None,
               retries_scheduled: 0,
               retries_rejected: 0 }
    }

    /// Crea un builder genérico para armar un flujo tipado paso a paso con
    /// stores personalizados. Enforcea en tiempo de compilación la
    /// compatibilidad de IO entre steps adyacentes.
    pub fn new_builder_with_stores(event_store: E, repository: R) -> EngineBuilderInit<E, R> {
        EngineBuilderInit { event_store, repository }
    }

    /// Crea un nuevo motor con una definición por defecto provista.
    /// Genera además un `flow_id` por defecto, habilitando `*_default_flow()`
    /// sin configuración extra.
    pub fn new_with_definition(event_store: E, repository: R, definition: crate::repo::FlowDefinition) -> Self {
        Self { event_store,
               repository,
               artifact_store: HashMap::new(),
               default_definition: Some(definition),
               default_flow_id: Some(Uuid::new_v4()),
               default_flow_name: None,
               retries_scheduled: 0,
               retries_rejected: 0 }
    }

    /// Crea un nuevo motor recibiendo directamente los steps y construyendo
    /// automáticamente la `FlowDefinition` (derivando ids de cada step).
    pub fn new_with_steps(event_store: E, repository: R, steps: Vec<Box<dyn crate::step::StepDefinition>>) -> Self {
        let definition = crate::repo::build_flow_definition_auto(steps);
        Self { event_store,
               repository,
               artifact_store: HashMap::new(),
               default_definition: Some(definition),
               default_flow_id: Some(Uuid::new_v4()),
               default_flow_name: None,
               retries_scheduled: 0,
               retries_rejected: 0 }
    }

    /// Igual que `new_with_steps`, pero además define un `flow_id` generado y
    /// asigna un nombre.
    pub fn new_with_steps_named(event_store: E,
                                repository: R,
                                flow_name: impl Into<String>,
                                steps: Vec<Box<dyn crate::step::StepDefinition>>)
                                -> Self {
        let mut engine = Self::new_with_steps(event_store, repository, steps);
        engine.default_flow_id = Some(Uuid::new_v4());
        engine.default_flow_name = Some(flow_name.into());
        engine
    }

    /// Establece/actualiza la definición por defecto del motor.
    pub fn set_default_definition(&mut self, definition: crate::repo::FlowDefinition) {
        self.default_definition = Some(definition);
    }

    /// Establece/actualiza la definición por defecto a partir de steps.
    pub fn set_default_steps(&mut self, steps: Vec<Box<dyn crate::step::StepDefinition>>) {
        let definition = crate::repo::build_flow_definition_auto(steps);
        self.default_definition = Some(definition);
    }

    /// Define/genera un `flow_id` por defecto si no existe aún.
    pub fn ensure_default_flow_id(&mut self) -> Uuid {
        if self.default_flow_id.is_none() {
            self.default_flow_id = Some(Uuid::new_v4());
        }
        self.default_flow_id.unwrap()
    }

    /// Fija explícitamente un `flow_id` por defecto.
    pub fn set_default_flow_id(&mut self, flow_id: Uuid) {
        self.default_flow_id = Some(flow_id);
    }

    /// Fija o actualiza el nombre descriptivo del flow por defecto.
    pub fn set_default_flow_name(&mut self, name: impl Into<String>) {
        self.default_flow_name = Some(name.into());
    }

    /// Obtiene el `flow_id` por defecto si está configurado.
    pub fn default_flow_id(&self) -> Option<Uuid> {
        self.default_flow_id
    }

    /// Obtiene el nombre del flow por defecto si está configurado.
    pub fn default_flow_name(&self) -> Option<&str> {
        self.default_flow_name.as_deref()
    }

    /// Crea un contexto para un `flow_id` y `definition` específicos.
    /// Este wrapper expone métodos que no requieren volver a pasar `flow_id` ni
    /// `definition`.
    pub fn with_flow<'a>(&'a mut self, flow_id: Uuid, definition: &'a FlowDefinition) -> FlowCtx<'a, E, R> {
        FlowCtx { engine: self,
                  flow_id,
                  definition }
    }

    // Nota: No se expone `with_default_flow` para evitar aliasing de préstamos
    // (&mut self con &self.definition).

    pub fn next_with(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
        let instance = self.load_or_init(flow_id, definition);
        let step_index = self.validate_state(&instance, definition)?;
        let (ctx, fingerprint, step_id) = self.prepare_context(&instance, definition, step_index)?;
        // Emit StepStarted antes de ejecutar.
        self.event_store.append_kind(flow_id,
                                     FlowEventKind::StepStarted { step_index,
                                                                  step_id: step_id.clone() });
        let step_def = &definition.steps[step_index];
        let outcome = self.execute_step(step_def.as_ref(), ctx, fingerprint.clone());
        self.persist_events(flow_id, step_index, &step_id, &outcome);
        // Si terminó el flow con éxito, emitir FlowCompleted con flow_fingerprint
        // agregado.
        if self.is_flow_completed_successfully(flow_id, definition) {
            let flow_fingerprint = self.compute_flow_fingerprint(flow_id);
            self.event_store
                .append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint });
        }
        // refrescar instancia no necesario para F2 (stateless en llamada)
        Ok(())
    }

    fn load_or_init(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> FlowInstance {
        let events = self.event_store.list(flow_id);
        if events.is_empty() {
            self.event_store
                .append_kind(flow_id,
                             FlowEventKind::FlowInitialized { definition_hash: definition.definition_hash.clone(),
                                                              step_count: definition.len() });
        }
        let events2 = self.event_store.list(flow_id);
        self.repository.load(flow_id, &events2, definition)
    }

    fn validate_state(&self, instance: &FlowInstance, definition: &FlowDefinition) -> Result<usize, CoreEngineError> {
        if instance.completed {
            return Err(CoreEngineError::FlowCompleted);
        }
        if instance.steps.iter().any(|s| matches!(s.status, StepStatus::Failed)) {
            return Err(CoreEngineError::FlowHasFailed);
        }
        let idx = instance.cursor;
        if idx >= definition.len() {
            return Err(CoreEngineError::StepAlreadyTerminal);
        }
        if !matches!(instance.steps[idx].status, StepStatus::Pending) {
            return Err(CoreEngineError::StepAlreadyTerminal);
        }
        Ok(idx)
    }

    fn prepare_context(&self,
                       instance: &FlowInstance,
                       definition: &FlowDefinition,
                       step_index: usize)
                       -> Result<(ExecutionContext, String, String), CoreEngineError> {
        let step_def = &definition.steps[step_index];
        if step_index == 0 && !matches!(step_def.kind(), crate::step::StepKind::Source) {
            return Err(CoreEngineError::FirstStepMustBeSource);
        }
        let input_artifact: Option<Artifact> = if step_index == 0 {
            None
        } else {
            let prev = &instance.steps[step_index - 1];
            if !matches!(prev.status, StepStatus::FinishedOk) {
                None
            } else {
                prev.outputs.first().and_then(|h| self.artifact_store.get(h)).cloned()
            }
        };
        if step_index > 0 && input_artifact.is_none() {
            return Err(CoreEngineError::MissingInputs);
        }
        let params = step_def.base_params();
        let mut input_hashes: Vec<String> = input_artifact.iter().map(|a| a.hash.clone()).collect();
        input_hashes.sort();
        let fingerprint = compute_step_fingerprint(step_def.id(), &input_hashes, &params, &definition.definition_hash);
        let ctx = ExecutionContext { input: input_artifact,
                                     params };
        Ok((ctx, fingerprint, step_def.id().to_string()))
    }

    fn execute_step(&mut self,
                    step_def: &dyn crate::step::StepDefinition,
                    ctx: ExecutionContext,
                    fingerprint: String)
                    -> ExecutionOutcome {
        match step_def.run(&ctx) {
            StepRunResult::Success { mut outputs } => {
                let output_hashes = self.hash_and_store_outputs(&mut outputs);
                ExecutionOutcome { fingerprint,
                                   output_hashes,
                                   signals: vec![],
                                   status: ExecutionStatus::Success }
            }
            StepRunResult::SuccessWithSignals { mut outputs, signals } => {
                let output_hashes = self.hash_and_store_outputs(&mut outputs);
                ExecutionOutcome { fingerprint,
                                   output_hashes,
                                   signals,
                                   status: ExecutionStatus::Success }
            }
            StepRunResult::Failure { error } => ExecutionOutcome { fingerprint,
                                                                   output_hashes: vec![],
                                                                   signals: vec![],
                                                                   status: ExecutionStatus::Failure(error) },
        }
    }

    fn persist_events(&mut self, flow_id: Uuid, step_index: usize, step_id: &str, outcome: &ExecutionOutcome) {
        // Emitir señales sólo en éxito. Si una señal es la reservada
        // PROPERTY_PREFERENCE_ASSIGNED con el payload esperado, traducirla al
        // evento fuerte PropertyPreferenceAssigned (F6), antes de StepFinished.
        // Además, si existe tal señal, incorporamos su params_hash al fingerprint
        // efectivo del Step (definición F6): el fingerprint del step sólo cambia
        // si cambian los parámetros o la política utilizada.
        let mut policy_params_hash: Option<String> = None;
        if matches!(outcome.status, ExecutionStatus::Success) {
            for StepSignal { signal, data } in outcome.signals.iter().cloned() {
                if signal == "PROPERTY_PREFERENCE_ASSIGNED" {
                    let property_key = data.get("property_key").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let policy_id = data.get("policy_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let params_hash = data.get("params_hash").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let rationale = data.get("rationale").cloned().unwrap_or_else(|| serde_json::json!({}));
                    if let (Some(property_key), Some(policy_id), Some(params_hash)) = (property_key, policy_id, params_hash) {
                        if policy_params_hash.is_none() {
                            policy_params_hash = Some(params_hash.clone());
                        }
                        self.event_store
                            .append_kind(flow_id,
                                         FlowEventKind::PropertyPreferenceAssigned { property_key,
                                                                                      policy_id,
                                                                                      params_hash,
                                                                                      rationale });
                        continue;
                    }
                }
                self.event_store.append_kind(flow_id,
                                             FlowEventKind::StepSignal { step_index,
                                                                         step_id: step_id.to_string(),
                                                                         signal,
                                                                         data });
            }
        }
        match &outcome.status {
            ExecutionStatus::Success => {
                // Fingerprint efectivo: base o mezclado con params_hash si hubo política
                let effective_fp = if let Some(ph) = policy_params_hash {
                    let mix = serde_json::json!({"base": outcome.fingerprint, "policy_params_hash": ph});
                    let canonical = to_canonical_json(&mix);
                    hash_str(&canonical)
                } else {
                    outcome.fingerprint.clone()
                };
                self.event_store.append_kind(flow_id,
                                             FlowEventKind::StepFinished { step_index,
                                                                           step_id: step_id.to_string(),
                                                                           outputs: outcome.output_hashes.clone(),
                                                                           fingerprint: effective_fp });
            }
            ExecutionStatus::Failure(err) => {
                self.event_store.append_kind(flow_id,
                                             FlowEventKind::StepFailed { step_index,
                                                                         step_id: step_id.to_string(),
                                                                         error: err.clone(),
                                                                         fingerprint: outcome.fingerprint.clone() });
            }
        }
        // FlowCompleted emitido por caller luego de verificar todos FinishedOk.
    }

    fn is_flow_completed_successfully(&self, flow_id: Uuid, definition: &FlowDefinition) -> bool {
        let events = self.event_store.list(flow_id);
        // No volver a emitir si ya existe FlowCompleted
        if events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowCompleted { .. })) {
            return false;
        }
        // Determinar el estado final vía replay (considera RetryScheduled y
        // transiciones Failed→Pending→FinishedOk). Un fallo previo no impide
        // completar si el estado actual de todos los steps es FinishedOk.
        let instance = self.repository.load(flow_id, &events, definition);
        instance.steps.iter().all(|s| matches!(s.status, StepStatus::FinishedOk))
    }

    fn compute_flow_fingerprint(&self, flow_id: Uuid) -> String {
        let events = self.event_store.list(flow_id);
        let mut fps: Vec<String> = events.iter()
                                         .filter_map(|e| match &e.kind {
                                             FlowEventKind::StepFinished { fingerprint, .. } => Some(fingerprint.clone()),
                                             _ => None,
                                         })
                                         .collect();
        fps.sort();
        let v = serde_json::Value::Array(fps.into_iter().map(serde_json::Value::String).collect());
        let canonical = to_canonical_json(&v);
        hash_str(&canonical)
    }

    #[cfg(test)]
    pub(crate) fn test_compute_flow_fingerprint(&self, flow_id: Uuid) -> String {
        self.compute_flow_fingerprint(flow_id)
    }
    fn hash_and_store_outputs(&mut self, outputs: &mut [Artifact]) -> Vec<String> {
        let mut output_hashes = Vec::new();
        for o in outputs.iter_mut() {
            let payload_canonical = to_canonical_json(&o.payload);
            let computed = hash_str(&payload_canonical);
            if o.hash.is_empty() {
                o.hash = computed.clone();
            }
            debug_assert_eq!(o.hash, computed, "Artifact hash debe ser hash(canonical_json(payload))");
            self.artifact_store.insert(o.hash.clone(), o.clone());
            output_hashes.push(o.hash.clone());
        }
        output_hashes
    }
    pub fn get_artifact(&self, hash: &str) -> Option<&Artifact> {
        self.artifact_store.get(hash)
    }

    /// Recupera y decodifica un artifact del store como tipo fuerte `T`.
    pub fn get_typed_artifact<T: ArtifactSpec + Clone>(
        &self,
        hash: &str)
        -> Option<Result<TypedArtifact<T>, crate::model::ArtifactDecodeError>> {
        self.artifact_store.get(hash).map(|a| TypedArtifact::<T>::decode(a))
    }

    /// Devuelve el último evento `StepFinished` para `step_id` en el `flow_id`
    /// dado.
    ///
    /// Notas de diseño y determinismo:
    /// - Los eventos se consultan del `EventStore` en memoria (o persistente en
    ///   otras implementaciones) y se escanean en orden inverso para hallar el
    ///   más reciente.
    /// - No hay efectos secundarios; sólo lectura.
    /// - Es seguro y estable: si el flujo no ejecutó ese step, devuelve `None`.
    pub fn last_step_finished_event(&self, flow_id: Uuid, step_id: &str) -> Option<crate::event::FlowEvent> {
        // Obtenemos una copia local de la secuencia de eventos y buscamos desde el
        // final.
        let events = self.event_store.list(flow_id);
        events.into_iter()
              .rev()
              .find(|e| matches!(&e.kind, FlowEventKind::StepFinished { step_id: sid, .. } if sid == step_id))
    }

    /// Devuelve los hashes de outputs del último `StepFinished` de `step_id`.
    ///
    /// Conveniente para recuperar rápidamente artifacts generados por un step
    /// sin tener que navegar manualmente por la lista de eventos.
    pub fn last_step_output_hashes(&self, flow_id: Uuid, step_id: &str) -> Option<Vec<String>> {
        self.last_step_finished_event(flow_id, step_id).and_then(|e| match e.kind {
                                                           FlowEventKind::StepFinished { outputs, .. } => Some(outputs),
                                                           _ => None,
                                                       })
    }

    /// Recupera el PRIMER artifact de salida del último `StepFinished` de
    /// `step_id` y lo decodifica como tipo fuerte `T`.
    ///
    /// Recomendado para el patrón pipeline (F2) donde cada step emite 0..1
    /// artifacts encadenados. Si hay múltiples outputs, se toma el primero
    /// por convención.
    ///
    /// Devuelve:
    /// - `None` si el step no tiene `StepFinished` o no produjo outputs.
    /// - `Some(Ok(TypedArtifact<T>))` si la decodificación fue exitosa (kind /
    ///   versión / validación OK).
    /// - `Some(Err(_))` si hubo un error de decodificación tipada.
    pub fn last_step_output_typed_for<T: ArtifactSpec + Clone>(
        &self,
        flow_id: Uuid,
        step_id: &str)
        -> Option<Result<TypedArtifact<T>, crate::model::ArtifactDecodeError>> {
        let first_hash = self.last_step_output_hashes(flow_id, step_id).and_then(|v| {
                                                                           // Mantener comportamiento predecible: usamos el
                                                                           // primer hash según el orden registrado.
                                                                           v.into_iter().next()
                                                                       });
        match first_hash {
            Some(h) => self.get_typed_artifact::<T>(&h),
            None => None,
        }
    }

    // -------------------------------------------------------------
    // Utilidades de alto nivel para consultar eventos (lectura pura)
    // -------------------------------------------------------------

    /// Devuelve todos los eventos del flujo (copia defensiva).
    /// Útil para depuración, inspección y testing sin acoplarse al store.
    pub fn events_for(&self, flow_id: Uuid) -> Vec<crate::event::FlowEvent> {
        self.event_store.list(flow_id)
    }

    /// Devuelve el primer evento (si existe).
    pub fn first_event(&self, flow_id: Uuid) -> Option<crate::event::FlowEvent> {
        self.event_store.list(flow_id).into_iter().next()
    }

    /// Devuelve el último evento (si existe).
    pub fn last_event(&self, flow_id: Uuid) -> Option<crate::event::FlowEvent> {
        self.event_store.list(flow_id).into_iter().next_back()
    }

    /// Busca el fingerprint agregado del flujo (producido por FlowCompleted).
    /// Retorna None si aún no se ha completado.
    /// El fingerpoint es un UUIDv4 codificado en base62 que se
    pub fn flow_fingerprint_for(&self, flow_id: Uuid) -> Option<String> {
        self.event_store.list(flow_id).into_iter().rev().find_map(|e| match e.kind {
                                                            FlowEventKind::FlowCompleted { flow_fingerprint } => {
                                                                Some(flow_fingerprint)
                                                            }
                                                            _ => None,
                                                        })
    }

    /// Devuelve el fingerprint del último StepFinished para `step_id`.
    pub fn last_step_fingerprint(&self, flow_id: Uuid, step_id: &str) -> Option<String> {
        self.event_store.list(flow_id).into_iter().rev().find_map(|e| match e.kind {
                                                            FlowEventKind::StepFinished { step_id: sid,
                                                                                          fingerprint,
                                                                                          .. } if sid == step_id => {
                                                                Some(fingerprint)
                                                            }
                                                            _ => None,
                                                        })
    }

    /// Devuelve la sub-secuencia de eventos asociados a un `step_id`
    /// (Started/Finished/Failed/Signal), en orden.
    pub fn step_events(&self, flow_id: Uuid, step_id: &str) -> Vec<crate::event::FlowEvent> {
        self.event_store
            .list(flow_id)
            .into_iter()
            .filter(|e| match &e.kind {
                FlowEventKind::StepStarted { step_id: sid, .. } if sid == step_id => true,
                FlowEventKind::StepFinished { step_id: sid, .. } if sid == step_id => true,
                FlowEventKind::StepFailed { step_id: sid, .. } if sid == step_id => true,
                FlowEventKind::StepSignal { step_id: sid, .. } if sid == step_id => true,
                _ => false,
            })
            .collect()
    }

    /// Representa la secuencia de eventos como una lista compacta de variantes
    /// "I,S,F,X,G,C". Conveniente para asserts de forma en tests.
    pub fn event_variants_for(&self, flow_id: Uuid) -> Vec<&'static str> {
        self.event_store
            .list(flow_id)
            .iter()
            .map(|e| match &e.kind {
                FlowEventKind::FlowInitialized { .. } => "I",
                FlowEventKind::StepStarted { .. } => "S",
                FlowEventKind::StepFinished { .. } => "F",
                FlowEventKind::StepFailed { .. } => "X",
                FlowEventKind::StepSignal { .. } => "G",
                FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
                FlowEventKind::RetryScheduled { .. } => "R",
                FlowEventKind::BranchCreated { .. } => "B",
                FlowEventKind::FlowCompleted { .. } => "C",
            })
            .collect()
    }

    /// Busca la última señal `signal` emitida por un `step_id`.
    pub fn last_signal(&self, flow_id: Uuid, step_id: &str, signal: &str) -> Option<crate::event::FlowEvent> {
        self.event_store
            .list(flow_id)
            .into_iter()
            .rev()
            .find(|e| matches!(&e.kind, FlowEventKind::StepSignal { step_id: sid, signal: s, .. } if sid == step_id && s == signal))
    }

    /// Devuelve el primer hash de output del último StepFinished de `step_id`.
    /// Alias práctico de `last_step_output_hashes(...).first()`.
    pub fn last_output_hash(&self, flow_id: Uuid, step_id: &str) -> Option<String> {
        self.last_step_output_hashes(flow_id, step_id)
            .and_then(|v| v.into_iter().next())
    }

    /// Ejecuta `next` repetidamente hasta completar el flujo o encontrar un
    /// error terminal. Útil para ejemplos y tests donde se quiere un run
    /// end-to-end con una sola llamada.
    pub fn run_to_end_with(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
        loop {
            let events_before = self.event_store.list(flow_id).len();
            match self.next_with(flow_id, definition) {
                Ok(()) => {
                    // Si no se agregó ningún evento nuevo, asumimos que no hay más trabajo.
                    let events_after = self.event_store.list(flow_id).len();
                    if events_after == events_before {
                        break;
                    }
                    // Si ya está completo, terminamos igualmente.
                    if self.is_flow_completed_successfully(flow_id, definition) {
                        break;
                    }
                }
                Err(CoreEngineError::FlowCompleted) => break,
                Err(CoreEngineError::FlowHasFailed) => return Err(CoreEngineError::FlowHasFailed),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Igual que `next`, pero usa la definición por defecto configurada en el
    /// engine. Paniquea si no hay definición por defecto.
    pub fn next_default(&mut self, flow_id: Uuid) -> Result<(), CoreEngineError> {
        // Mover temporalmente la definición fuera para evitar préstamos simultáneos (&
        // y &mut).
        let def = self.default_definition.take().expect("FlowEngine default_definition not configured. Use new_with_steps/new_with_definition or set_default_*.");
        let res = self.next_with(flow_id, &def);
        self.default_definition = Some(def);
        res
    }

    /// Ejecuta hasta completar usando la definición por defecto.
    pub fn run_to_end_default(&mut self, flow_id: Uuid) -> Result<(), CoreEngineError> {
        let def = self.default_definition.take().expect("FlowEngine default_definition not configured. Use new_with_steps/new_with_definition or set_default_*.");
        let res = self.run_to_end_with(flow_id, &def);
        self.default_definition = Some(def);
        res
    }

    /// Igual que `next_default`, pero usando/eligiendo internamente un
    /// `flow_id` por defecto.
    pub fn next_default_flow(&mut self) -> Result<Uuid, CoreEngineError> {
        let flow_id = self.ensure_default_flow_id();
        self.next_default(flow_id)?;
        Ok(flow_id)
    }

    /// Ejecuta hasta completar sobre el `flow_id` por defecto.
    pub fn run_to_end_default_flow(&mut self) -> Result<Uuid, CoreEngineError> {
        let flow_id = self.ensure_default_flow_id();
        self.run_to_end_default(flow_id)?;
        Ok(flow_id)
    }

    /// Eventos del flow por defecto.
    pub fn events_default(&self) -> Option<Vec<crate::event::FlowEvent>> {
        self.default_flow_id.map(|id| self.events_for(id))
    }

    /// Fingerprint del flow por defecto si está configurado.
    pub fn flow_fingerprint_default(&self) -> Option<String> {
        self.default_flow_id.and_then(|id| self.flow_fingerprint_for(id))
    }

    /// Secuencia compacta de variantes (I,S,F,X,G,C) para el flow por defecto.
    pub fn event_variants_default(&self) -> Option<Vec<&'static str>> {
        self.default_flow_id.map(|id| self.event_variants_for(id))
    }

    /// Último output tipado del step `step_id` sobre el flow por defecto.
    pub fn last_step_output<T: ArtifactSpec + Clone>(
        &self,
        step_id: &str)
        -> Option<Result<TypedArtifact<T>, crate::model::ArtifactDecodeError>> {
        self.default_flow_id
            .and_then(|id| self.last_step_output_typed_for::<T>(id, step_id))
    }

    /// Alias compatible: mismo comportamiento que `last_step_output`, mantiene
    /// el nombre previo usado en ejemplos.
    pub fn last_step_output_typed<T: ArtifactSpec + Clone>(
        &self,
        step_id: &str)
        -> Option<Result<TypedArtifact<T>, crate::model::ArtifactDecodeError>> {
        self.last_step_output::<T>(step_id)
    }

    // --- Nuevos atajos sin flow_id: aplican a cualquier backend ---
    /// Devuelve todos los eventos del flow por defecto, si está configurado.
    /// Azúcar sintáctico sobre `events_default()` para una API menos verbosa.
    pub fn events(&self) -> Option<Vec<crate::event::FlowEvent>> {
        self.events_default()
    }

    /// Devuelve variantes de eventos (I,S,F,X,G,C) del flow por defecto.
    pub fn event_variants(&self) -> Option<Vec<&'static str>> {
        self.event_variants_default()
    }

    /// Devuelve el fingerprint agregado del flow por defecto (si existe).
    pub fn flow_fingerprint(&self) -> Option<String> {
        self.flow_fingerprint_default()
    }

    // ------------------------------
    // F7: API de reintento manual
    // ------------------------------
    /// Agenda un reintento para `step_id` si el estado actual del flow lo permite.
    ///
    /// Regla:
    /// - Sólo si el `step_id` está en estado Failed.
    /// - Al emitir `RetryScheduled`, el replay marcará ese step como Pending
    ///   (Failed→Pending) permitiendo una re-ejecución en la próxima llamada a `next`.
    /// - Esta operación no altera fingerprints.
    pub fn schedule_retry(&mut self,
                          flow_id: Uuid,
                          definition: &FlowDefinition,
                          step_id: &str,
                          reason: Option<String>,
                          max_retries: Option<u32>) -> Result<bool, CoreEngineError> {
        let instance = self.load_or_init(flow_id, definition);
        // Buscar índice del step por id
        let idx_opt = instance.steps.iter().position(|s| s.step_id == step_id);
        let idx = match idx_opt { Some(i) => i, None => return Err(CoreEngineError::InvalidStepIndex) };
        let slot = &instance.steps[idx];
        // Debe estar Failed
        if !matches!(slot.status, StepStatus::Failed) {
            self.retries_rejected += 1;
            return Ok(false);
        }
        // Política: verificar límites si se proveen
        if let Some(max) = max_retries {
            if slot.retry_count >= max {
                self.retries_rejected += 1;
                return Ok(false);
            }
        }
        let retry_index = slot.retry_count + 1;
        self.event_store
            .append_kind(flow_id,
                         FlowEventKind::RetryScheduled { step_id: step_id.to_string(),
                                                          retry_index,
                                                          reason });
        self.retries_scheduled += 1;
        Ok(true)
    }

    /// Métricas internas (F7): cantidad de reintentos agendados.
    pub fn retries_scheduled(&self) -> u64 { self.retries_scheduled }
    /// Métricas internas (F7): cantidad de reintentos rechazados.
    pub fn retries_rejected(&self) -> u64 { self.retries_rejected }

    /// Crea una rama (branch) a partir del estado actual del `flow_id` en el
    /// step `from_step_id` especificado. Reglas (simplificadas para F9):
    /// - Sólo se permite branch sobre steps con estado `FinishedOk`.
    /// - Genera un `branch_id` (UUID) y emite un evento `BranchCreated` con
    ///   metadatos mínimos. El `divergence_params_hash` es opcional y puede ser
    ///   calculado por capas superiores.
    /// - No replica ni duplica eventos futuros; simplemente registra la rama
    ///   y su evento para auditoría.
    pub fn branch(&mut self,
                  flow_id: Uuid,
                  definition: &FlowDefinition,
                  from_step_id: &str,
                  divergence_params_hash: Option<String>) -> Result<Uuid, CoreEngineError> {
        let instance = self.load_or_init(flow_id, definition);
        // Buscar índice del step por id
        let idx_opt = instance.steps.iter().position(|s| s.step_id == from_step_id);
        let idx = match idx_opt { Some(i) => i, None => return Err(CoreEngineError::InvalidStepIndex) };
        let slot = &instance.steps[idx];
        if !matches!(slot.status, StepStatus::FinishedOk) {
            return Err(CoreEngineError::InvalidBranchSource);
        }
        // Generar id de rama y root flow id (por ahora usamos flow_id como root)
        let branch_id = Uuid::new_v4();
        let root_flow_id = flow_id; // en ausencia de tabla root detection, use flow_id como root

        // Clon parcial: copiar eventos hasta el StepFinished que corresponde a `from_step_id`
        // Esto crea un nuevo flujo (branch_id) que contiene el histórico hasta ese paso.
        let events = self.event_store.list(flow_id);
        // Encontrar último índice de evento StepFinished para from_step_id
        let mut last_idx: Option<usize> = None;
        for (i, e) in events.iter().enumerate() {
            if let FlowEventKind::StepFinished { step_id, .. } = &e.kind {
                if step_id == from_step_id {
                    last_idx = Some(i);
                }
            }
        }
        // Si no encontramos un StepFinished para el step (aunque el slot indica FinishedOk), fallback: copy up to FlowInitialized
        let copy_up_to = match last_idx {
            Some(i) => i + 1, // inclusive
            None => {
                // try to copy at least FlowInitialized if present
                if events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })) {
                    1
                } else { 0 }
            }
        };

        if copy_up_to > 0 {
            for ev in events.into_iter().take(copy_up_to) {
                // append_kind consumes a FlowEventKind, so clone the kind
                self.event_store.append_kind(branch_id, ev.kind.clone());
            }
        }

        // Emitir BranchCreated en el flujo padre (persistencia insertará metadata en workflow_branches)
        self.event_store.append_kind(flow_id,
                                     FlowEventKind::BranchCreated {
                                         branch_id,
                                         parent_flow_id: flow_id,
                                         root_flow_id,
                                         created_from_step_id: from_step_id.to_string(),
                                         divergence_params_hash,
                                     });
        Ok(branch_id)
    }
}

// -------------------------------------------------------------
// API ergonómica (especialización) para InMemoryEventStore +
// InMemoryFlowRepository Ruta preferida: new(definition) + next() sin
// parámetros. -------------------------------------------------------------
impl FlowEngine<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository> {
    #[allow(clippy::new_ret_no_self)]
    /// Constructor ergonómico del builder tipado para uso in-memory:
    /// FlowEngine::new().firstStep(...).addStep(...).build()
    pub fn new() -> EngineBuilderInit<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository> {
        EngineBuilderInit { event_store: crate::event::InMemoryEventStore::default(),
                            repository: crate::repo::InMemoryFlowRepository::new() }
    }

    /// Alternativa: crear un motor directamente desde una definición (con
    /// flow_id generado).
    pub fn from_definition(definition: crate::repo::FlowDefinition) -> Self {
        Self { event_store: crate::event::InMemoryEventStore::default(),
               repository: crate::repo::InMemoryFlowRepository::new(),
               artifact_store: HashMap::new(),
               default_definition: Some(definition),
               default_flow_id: Some(Uuid::new_v4()),
               default_flow_name: None,
               retries_scheduled: 0,
               retries_rejected: 0 }
    }

    /// Asigna un nombre descriptivo al flow por defecto (builder-style).
    pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.default_flow_name = Some(name.into());
        self
    }

    /// Ejecuta el siguiente step usando la definición e id por defecto.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Uuid, CoreEngineError> {
        let id = self.ensure_default_flow_id();
        self.next_default(id)?;
        Ok(id)
    }

    /// Ejecuta hasta completar usando la definición e id por defecto.
    pub fn run_to_end(&mut self) -> Result<Uuid, CoreEngineError> {
        let id = self.ensure_default_flow_id();
        self.run_to_end_default(id)?;
        Ok(id)
    }

    // Nota: métodos de lectura sin `flow_id` ahora están disponibles en la
    // impl genérica. Mantener sólo los métodos de ejecución ergonómicos aquí.
}

impl<'a, E: EventStore, R: FlowRepository> FlowCtx<'a, E, R> {
    /// Ejecuta el siguiente step del flujo.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<(), CoreEngineError> {
        self.engine.next_with(self.flow_id, self.definition)
    }

    /// Ejecuta hasta completar el flujo (o hasta error terminal).
    pub fn run_to_end(&mut self) -> Result<(), CoreEngineError> {
        self.engine.run_to_end_with(self.flow_id, self.definition)
    }

    /// Eventos del flujo.
    pub fn events(&self) -> Vec<crate::event::FlowEvent> {
        self.engine.events_for(self.flow_id)
    }

    /// Secuencia compacta de variantes (I,S,F,X,G,C).
    pub fn event_variants(&self) -> Vec<&'static str> {
        self.engine.event_variants_for(self.flow_id)
    }

    /// Fingerprint agregado si el flujo está completado.
    pub fn flow_fingerprint(&self) -> Option<String> {
        self.engine.flow_fingerprint_for(self.flow_id)
    }

    /// Último output (primer hash) de un step, decodificado como tipo fuerte
    /// `T`.
    pub fn last_step_output_typed<T: ArtifactSpec + Clone>(
        &self,
        step_id: &str)
        -> Option<Result<TypedArtifact<T>, crate::model::ArtifactDecodeError>> {
        self.engine.last_step_output_typed_for::<T>(self.flow_id, step_id)
    }

    /// Hash del primer output del último StepFinished de `step_id`.
    pub fn last_output_hash(&self, step_id: &str) -> Option<String> {
        self.engine.last_output_hash(self.flow_id, step_id)
    }

    /// Eventos asociados a un step.
    pub fn step_events(&self, step_id: &str) -> Vec<crate::event::FlowEvent> {
        self.engine.step_events(self.flow_id, step_id)
    }

    /// Fingerprint del último StepFinished para `step_id`.
    pub fn last_step_fingerprint(&self, step_id: &str) -> Option<String> {
        self.engine.last_step_fingerprint(self.flow_id, step_id)
    }
}

/// Helper recomendado por especificación (Sección 17) para encapsular cálculo
/// fingerprint.
pub fn compute_step_fingerprint(step_id: &str,
                                input_hashes: &[String],
                                params: &serde_json::Value,
                                definition_hash: &str)
                                -> String {
    let fp_input = StepFingerprintInput { engine_version: ENGINE_VERSION,
                                          step_id,
                                          input_hashes,
                                          params,
                                          definition_hash };
    let fp_json = serde_json::to_value(&fp_input).expect("fingerprint serialize");
    let canonical = to_canonical_json(&fp_json);
    hash_str(&canonical)
}

// -------------------------------------------------------------
// Builder tipado del Engine – enforces N::Input == Prev::Output
// -------------------------------------------------------------
use crate::step::{SameAs, StepDefinition, TypedStep};
use std::marker::PhantomData;

/// Estado inicial del builder: requiere definir el primer step.
pub struct EngineBuilderInit<E: EventStore, R: FlowRepository> {
    event_store: E,
    repository: R,
}

impl<E: EventStore, R: FlowRepository> EngineBuilderInit<E, R> {
    /// Define el primer step (debe ser Source). Devuelve un builder tipado.
    pub fn first_step<S>(self, step: S) -> EngineBuilder<S, E, R>
        where S: TypedStep + 'static
    {
        // Validación temprana: el primer step debe ser de tipo Source.
        if !matches!(step.kind(), crate::step::StepKind::Source) {
            panic!("El primer step debe ser de tipo Source");
        }
        let steps: Vec<Box<dyn StepDefinition>> = vec![Box::new(step)];
        EngineBuilder { event_store: self.event_store,
                        repository: self.repository,
                        steps,
                        _out: PhantomData::<S::Output> }
    }

    /// Alias en camelCase solicitado: firstStep
    #[allow(non_snake_case)]
    pub fn firstStep<S>(self, step: S) -> EngineBuilder<S, E, R>
        where S: TypedStep + 'static
    {
        self.first_step(step)
    }
}

/// Builder tipado con al menos un step definido.
pub struct EngineBuilder<S: TypedStep + 'static, E: EventStore, R: FlowRepository> {
    event_store: E,
    repository: R,
    steps: Vec<Box<dyn StepDefinition>>,
    _out: PhantomData<<S as TypedStep>::Output>,
}

impl<S: TypedStep + 'static, E: EventStore, R: FlowRepository> EngineBuilder<S, E, R> {
    /// Agrega un step verificando en compilación que la entrada del siguiente
    /// coincide con la salida del previo.
    pub fn add_step_internal<N>(mut self, next: N) -> EngineBuilder<N, E, R>
        where N: TypedStep + 'static,
              <N as TypedStep>::Input: SameAs<<S as TypedStep>::Output>
    {
        self.steps.push(Box::new(next));
        EngineBuilder::<N, E, R> { event_store: self.event_store,
                                   repository: self.repository,
                                   steps: self.steps,
                                   _out: PhantomData }
    }

    /// Alias en camelCase solicitado: addStep
    pub fn add_step<N>(self, next: N) -> EngineBuilder<N, E, R>
        where N: TypedStep + 'static,
              <N as TypedStep>::Input: SameAs<<S as TypedStep>::Output>
    {
        self.add_step_internal(next)
    }

    /// Construye el FlowEngine con la definición por defecto y flow_id
    /// generado.
    pub fn build(self) -> FlowEngine<E, R> {
        // Build definition (ids derivan del trait TypedStep via adaptador existente)
        let definition = crate::repo::build_flow_definition_auto(self.steps);
        // Validación temprana opcional: primer step debe ser Source
        debug_assert!(matches!(definition.steps.first().map(|s| s.kind()),
                               Some(crate::step::StepKind::Source)),
                      "El primer step debe ser de tipo Source");
        FlowEngine { event_store: self.event_store,
                     repository: self.repository,
                     artifact_store: HashMap::new(),
                     default_definition: Some(definition),
                     default_flow_id: Some(Uuid::new_v4()),
                     default_flow_name: None,
                     retries_scheduled: 0,
                     retries_rejected: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArtifactKind, ArtifactSpec};
    use crate::{build_flow_definition, step::{StepDefinition, StepKind}, InMemoryEventStore, InMemoryFlowRepository, StepRunResult};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use uuid::Uuid;
    #[derive(Clone, Serialize, Deserialize)]
    struct SeedOutput {
        values: Vec<i64>,
        schema_version: u32,
    }
    impl ArtifactSpec for SeedOutput {
        const KIND: ArtifactKind = ArtifactKind::GenericJson;
    }

    /// Artifact producido por el step de transformación que suma los valores
    /// previos.
    #[derive(Clone, Serialize, Deserialize)]
    struct SumOutput {
        sum: i64,
        schema_version: u32,
    }
    impl ArtifactSpec for SumOutput {
        const KIND: ArtifactKind = ArtifactKind::GenericJson;
    }

    // -----------------------------------------------------------
    // STEPS (definen interfaz puramente determinista y neutral)
    // -----------------------------------------------------------
    /// Step fuente: no necesita inputs y genera un SeedOutput determinista.
    struct SeedStep;
    impl crate::step::StepDefinition for SeedStep {
        fn id(&self) -> &str {
            "seed"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({"n":2})
        } // Param dummy para fingerprint.
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            // Datos deterministas (no tiempo / random) => reproducibilidad garantizada.
            let data = SeedOutput { values: vec![1, 2],
                                    schema_version: 1 };
            let art = data.into_artifact(); // sin hash todavía; engine lo calcula.
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> crate::step::StepKind {
            crate::step::StepKind::Source
        }
    }

    /// Step transformador: consume el output del seed y produce la suma.
    struct SumStep;
    impl crate::step::StepDefinition for SumStep {
        fn id(&self) -> &str {
            "sum"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
            // Uso de tipado fuerte para deserializar el primer artifact.
            use crate::model::TypedArtifact;
            let first = ctx.input.as_ref().expect("seed output present");
            let seed = TypedArtifact::<SeedOutput>::decode(first).expect("decode seed");
            let s: i64 = seed.inner.values.iter().sum();
            let out = SumOutput { sum: s,
                                  schema_version: 1 };
            StepRunResult::Success { outputs: vec![out.into_artifact()] }
        }
        fn kind(&self) -> crate::step::StepKind {
            crate::step::StepKind::Transform
        }
    }

    // ------------------------- F6 tests: parity and sensitivity -------------------------
    struct PolicySrc { label: &'static str, params_hash: &'static str }
    impl StepDefinition for PolicySrc {
        fn id(&self) -> &str { self.label }
        fn base_params(&self) -> serde_json::Value { json!({}) }
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            let art = Artifact { kind: ArtifactKind::GenericJson,
                                 hash: String::new(),
                                 payload: json!({"schema_version":1, "demo":true}),
                                 metadata: None };
            let data = json!({
                "property_key": "inchikey:XYZ|prop:foo",
                "policy_id": "max_score",
                "params_hash": self.params_hash,
                "rationale": {"t": 1}
            });
            StepRunResult::SuccessWithSignals { outputs: vec![art],
                                                signals: vec![StepSignal { signal:
                                                                                   "PROPERTY_PREFERENCE_ASSIGNED".into(),
                                                                           data }] }
        }
        fn kind(&self) -> StepKind { StepKind::Source }
    }

    fn run_one(engine: &mut FlowEngine<InMemoryEventStore, InMemoryFlowRepository>, step: Box<dyn StepDefinition>) -> (Uuid, String) {
        let flow_id = Uuid::new_v4();
        // Evitar mover `step` mientras está prestado: primero capturamos el id,
        // luego construimos la definición moviendo `step` en una segunda sentencia.
        let step_id_owned = step.id().to_string();
        let ids = [step_id_owned.as_str()];
        let def = build_flow_definition(&ids, vec![step]);
        engine.next_with(flow_id, &def).expect("ok");
        let fp = engine.last_step_fingerprint(flow_id, def.steps[0].id()).expect("fp");
        (flow_id, fp)
    }

    #[test]
    fn f6_fp_parity_same_params_same_fp() {
    let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let (_id1, fp1) = run_one(&mut engine, Box::new(PolicySrc { label: "p", params_hash: "aaa" }));
        let (_id2, fp2) = run_one(&mut engine, Box::new(PolicySrc { label: "p", params_hash: "aaa" }));
        assert_eq!(fp1, fp2, "Fingerprint debe ser igual con mismos params_hash");
    }

    #[test]
    fn f6_fp_sensitivity_diff_params_diff_fp() {
    let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let (_id1, fp1) = run_one(&mut engine, Box::new(PolicySrc { label: "p", params_hash: "aaa" }));
        let (_id2, fp2) = run_one(&mut engine, Box::new(PolicySrc { label: "p", params_hash: "bbb" }));
        assert_ne!(fp1, fp2, "Fingerprint debe cambiar si cambia params_hash");
    }

    // -----------------------------------------------------------
    // TEST PRINCIPAL: DEMOSTRACIÓN DE DETERMINISMO
    // -----------------------------------------------------------
    /// Ejecuta el mismo flujo dos veces (con motores limpios) y verifica:
    /// 1. Secuencia de eventos (por tipo) idéntica.
    /// 2. Fingerprint del step final igual.
    /// Esto valida que la lógica es pura dado (definition + inputs + params).
    // ----------------------------------------------------------------------------------
    // TEST 1: Flujo determinista de dos steps (seed -> sum)
    // ----------------------------------------------------------------------------------
    #[test]
    fn deterministic_two_step_flow() {
        // Crear un UUID usado para ambos runs: permite comparar evento a evento.
        let flow_id = Uuid::new_v4();

        // Primer engine (run #1)
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps: Vec<Box<dyn StepDefinition>> = vec![Box::new(SeedStep), Box::new(SumStep)];
        let ids = ["seed", "sum"]; // Orden define el definition_hash
        let definition = build_flow_definition(&ids, steps);
        engine.next_with(flow_id, &definition).unwrap(); // step seed
        engine.next_with(flow_id, &definition).unwrap(); // step sum
        let events_run1 = engine.event_store.list(flow_id);

        // Segundo engine (run #2) – reconstruye sin reutilizar estado previo.
        let mut engine2 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps2: Vec<Box<dyn StepDefinition>> = vec![Box::new(SeedStep), Box::new(SumStep)];
        let definition2 = build_flow_definition(&ids, steps2);
        engine2.next_with(flow_id, &definition2).unwrap();
        engine2.next_with(flow_id, &definition2).unwrap();
        let events_run2 = engine2.event_store.list(flow_id);

        // Normalizar eventos a su nombre de variante (ignoramos timestamps y hashes
        // concretos).
    fn simplify(ev: &crate::event::FlowEventKind) -> String {
            match ev {
                crate::event::FlowEventKind::FlowInitialized { .. } => "FlowInitialized",
                crate::event::FlowEventKind::StepStarted { .. } => "StepStarted",
                crate::event::FlowEventKind::StepFinished { .. } => "StepFinished",
                crate::event::FlowEventKind::StepFailed { .. } => "StepFailed",
                crate::event::FlowEventKind::StepSignal { .. } => "StepSignal",
        crate::event::FlowEventKind::RetryScheduled { .. } => "RetryScheduled",
    crate::event::FlowEventKind::PropertyPreferenceAssigned { .. } => "PropertyPreferenceAssigned",
        crate::event::FlowEventKind::BranchCreated { .. } => "BranchCreated",
        crate::event::FlowEventKind::FlowCompleted { .. } => "FlowCompleted",
            }.to_string()
        }
        let seq1: Vec<String> = events_run1.iter().map(|e| simplify(&e.kind)).collect();
        let seq2: Vec<String> = events_run2.iter().map(|e| simplify(&e.kind)).collect();
        assert_eq!(seq1, seq2, "Event sequences must match deterministically");

        // Obtener fingerprint del step final ("sum") en ambos runs y compararlos.
        let fp1 =
            events_run1.iter().find_map(|e| {
                                  if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind {
                                      if step_id == "sum" {
                                          Some(fingerprint.clone())
                                      } else {
                                          None
                                      }
                                  } else {
                                      None
                                  }
                              });
        let fp2 =
            events_run2.iter().find_map(|e| {
                                  if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind {
                                      if step_id == "sum" {
                                          Some(fingerprint.clone())
                                      } else {
                                          None
                                      }
                                  } else {
                                      None
                                  }
                              });
        assert_eq!(fp1, fp2, "Fingerprints must be stable");

        // Nota: Podríamos también validar que los hashes de artifacts
        // producidos coinciden. Si fingerprint es igual y la lógica es
        // pura, esa igualdad se mantiene.
    }

    // ----------------------------------------------------------------------------------
    // TEST 1b (G1 específico): Tres ejecuciones idénticas comparando secuencias de
    // eventos.
    // ----------------------------------------------------------------------------------
    #[test]
    fn determinism_three_runs_event_sequence() {
        let flow_id = Uuid::new_v4();
        let ids = ["seed", "sum"];
        // Run 1
        let mut e1 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def1 = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
        e1.next_with(flow_id, &def1).unwrap();
        e1.next_with(flow_id, &def1).unwrap();
        // Run 2
        let mut e2 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def2 = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
        e2.next_with(flow_id, &def2).unwrap();
        e2.next_with(flow_id, &def2).unwrap();
        // Run 3
        let mut e3 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def3 = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
        e3.next_with(flow_id, &def3).unwrap();
        e3.next_with(flow_id, &def3).unwrap();
        let seq = |evs: &[crate::event::FlowEvent]| {
            evs.iter()
               .map(|e| match &e.kind {
                   crate::event::FlowEventKind::FlowInitialized { .. } => "I",
                   crate::event::FlowEventKind::StepStarted { .. } => "S",
                   crate::event::FlowEventKind::StepFinished { .. } => "F",
                   crate::event::FlowEventKind::StepFailed { .. } => "X",
                   crate::event::FlowEventKind::StepSignal { .. } => "G", // generic signal
                   crate::event::FlowEventKind::RetryScheduled { .. } => "R",
                       crate::event::FlowEventKind::BranchCreated { .. } => "B",
                       crate::event::FlowEventKind::FlowCompleted { .. } => "C",
                       crate::event::FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
               })
               .collect::<Vec<_>>()
        };
        let s1 = seq(&e1.event_store.list(flow_id));
        let s2 = seq(&e2.event_store.list(flow_id));
        let s3 = seq(&e3.event_store.list(flow_id));
        assert_eq!(s1, s2, "Run1 vs Run2");
        assert_eq!(s2, s3, "Run2 vs Run3");
    }

    // ----------------------------------------------------------------------------------
    // TEST 1c (G2): Todos los fingerprints de todos los steps coinciden entre 3
    // runs.
    // ----------------------------------------------------------------------------------
    #[test]
    fn all_step_fingerprints_equal_across_three_runs() {
        let flow_id = Uuid::new_v4();
        let ids = ["seed", "sum"];
        let run = |flow_id| {
            let mut e = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
            let def = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
            e.next_with(flow_id, &def).unwrap();
            e.next_with(flow_id, &def).unwrap();
            e.event_store.list(flow_id)
        };
        let ev1 = run(flow_id);
        let ev2 = run(flow_id);
        let ev3 = run(flow_id);
        let fps = |evs: &[crate::event::FlowEvent]| {
            evs.iter()
               .filter_map(|e| {
                   if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind {
                       Some((step_id.clone(), fingerprint.clone()))
                   } else {
                       None
                   }
               })
               .collect::<Vec<_>>()
        };
        let f1 = fps(&ev1);
        let f2 = fps(&ev2);
        let f3 = fps(&ev3);
        assert_eq!(f1, f2, "Run1 vs Run2 fingerprints difieren");
        assert_eq!(f2, f3, "Run2 vs Run3 fingerprints difieren");
        assert_eq!(f1.len(), 2, "Deben existir fingerprints de los 2 steps");
    }

    // ----------------------------------------------------------------------------------
    // TEST 2: Flujo de un solo step (source). Verifica eventos y finalización.
    // ----------------------------------------------------------------------------------
    #[test]
    fn run_linear_single_step() {
        #[derive(Clone, Serialize, Deserialize)]
        struct SingleOut {
            v: i32,
            schema_version: u32,
        }
        impl ArtifactSpec for SingleOut {
            const KIND: ArtifactKind = ArtifactKind::GenericJson;
        }
        struct SingleStep;
        impl crate::step::StepDefinition for SingleStep {
            fn id(&self) -> &str {
                "single"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![SingleOut { v: 42,
                                                                   schema_version: 1 }.into_artifact()] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["single"], vec![Box::new(SingleStep)]);
        engine.next_with(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
    let variants: Vec<_> = events.iter()
                                     .map(|e| match &e.kind {
                                         crate::event::FlowEventKind::FlowInitialized { .. } => "I",
                                         crate::event::FlowEventKind::StepStarted { .. } => "S",
                                         crate::event::FlowEventKind::StepFinished { .. } => "F",
                                         crate::event::FlowEventKind::BranchCreated { .. } => "B",
                                         crate::event::FlowEventKind::FlowCompleted { .. } => "C",
                                         crate::event::FlowEventKind::StepFailed { .. } => "X",
                                         crate::event::FlowEventKind::StepSignal { .. } => "G",
                     crate::event::FlowEventKind::RetryScheduled { .. } => "R",
                                         crate::event::FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
                                     })
                                     .collect();
        assert_eq!(variants, vec!["I", "S", "F", "C"], "Secuencia esperada para un sólo step");
    }

    // ----------------------------------------------------------------------------------
    // TEST 3: Dos steps lineales (happy path) – verifica hashes de output no
    // vacíos.
    // ----------------------------------------------------------------------------------
    #[test]
    fn run_linear_two_steps() {
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps: Vec<Box<dyn StepDefinition>> = vec![Box::new(SeedStep), Box::new(SumStep)];
        let definition = build_flow_definition(&["seed", "sum"], steps);
        engine.next_with(flow_id, &definition).unwrap();
        engine.next_with(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
        let finished = events.iter()
                             .filter(|e| matches!(e.kind, crate::event::FlowEventKind::StepFinished { .. }))
                             .count();
        assert_eq!(finished, 2, "Deben terminar dos steps");
    }

    // ----------------------------------------------------------------------------------
    // TEST 4: Fingerprint estabilidad explícita (comparación directa string).
    // ----------------------------------------------------------------------------------
    #[test]
    fn fingerprint_stability() {
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["seed", "sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        engine.next_with(flow_id, &definition).unwrap();
        engine.next_with(flow_id, &definition).unwrap();
        let fp1 = engine.event_store.list(flow_id).iter().find_map(|e| {
                                                             if let crate::event::FlowEventKind::StepFinished { step_id,
                                                                                                                fingerprint,
                                                                                                                .. } =
                                                                 &e.kind
                                                             {
                                                                 if step_id == "sum" {
                                                                     Some(fingerprint.clone())
                                                                 } else {
                                                                     None
                                                                 }
                                                             } else {
                                                                 None
                                                             }
                                                         });
        // run 2
        let mut engine2 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition2 = build_flow_definition(&["seed", "sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        engine2.next_with(flow_id, &definition2).unwrap();
        engine2.next_with(flow_id, &definition2).unwrap();
        let fp2 = engine2.event_store.list(flow_id).iter().find_map(|e| {
                                                              if let crate::event::FlowEventKind::StepFinished { step_id,
                                                                                                                 fingerprint,
                                                                                                                 .. } =
                                                                  &e.kind
                                                              {
                                                                  if step_id == "sum" {
                                                                      Some(fingerprint.clone())
                                                                  } else {
                                                                      None
                                                                  }
                                                              } else {
                                                                  None
                                                              }
                                                          });
        assert_eq!(fp1, fp2, "Fingerprint debe coincidir");
    }

    // ----------------------------------------------------------------------------------
    // TEST 5: Fallo no avanza cursor (step 2 falla y no se re-ejecuta).
    // ----------------------------------------------------------------------------------
    #[test]
    fn failure_stops_following_steps() {
        struct FailStep; // siempre falla
        impl crate::step::StepDefinition for FailStep {
            fn id(&self) -> &str {
                "fail"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Failure { error: CoreEngineError::MissingInputs }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["seed", "fail"], vec![Box::new(SeedStep), Box::new(FailStep)]);
        engine.next_with(flow_id, &definition).unwrap(); // seed ok
        engine.next_with(flow_id, &definition).unwrap(); // fail step executes
                                                         // intentar de nuevo debe dar FlowHasFailed (stop-on-failure)
        let err = engine.next_with(flow_id, &definition).unwrap_err();
        assert_eq!(err.to_string(), crate::errors::CoreEngineError::FlowHasFailed.to_string());
    }

    // ----------------------------------------------------------------------------------
    // TEST F7: Retry manual – agenda RetryScheduled y verifica transición Failed→Pending
    // y que el fingerprint del StepFinished tras el retry es igual al que habría sido
    // sin fallar (mismos inputs/params).
    // ----------------------------------------------------------------------------------
    #[test]
    fn retry_scheduled_transitions_and_fp_stable() {
        // Step que falla la primera vez y luego tiene éxito
        use std::sync::{Arc, Mutex};
        #[derive(Clone)]
        struct Flaky { state: Arc<Mutex<u32>> }
        impl StepDefinition for Flaky {
            fn id(&self) -> &str { "flaky" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                let mut c = self.state.lock().unwrap();
                if *c == 0 {
                    *c = 1;
                    StepRunResult::Failure { error: CoreEngineError::Internal("boom".into()) }
                } else {
                    let art = serde_json::json!({"ok":true, "schema_version":1});
                    StepRunResult::Success { outputs: vec![Artifact { kind: ArtifactKind::GenericJson,
                                                                      hash: String::new(),
                                                                      payload: art,
                                                                      metadata: None }] }
                }
            }
            fn kind(&self) -> StepKind { StepKind::Transform }
        }
        struct Src;
        impl StepDefinition for Src {
            fn id(&self) -> &str { "src" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Artifact { kind: ArtifactKind::GenericJson,
                                                                  hash: String::new(),
                                                                  payload: json!({"schema_version":1}),
                                                                  metadata: None }] }
            }
            fn kind(&self) -> StepKind { StepKind::Source }
        }
        let flow_id = Uuid::new_v4();
        let flaky = Flaky { state: Arc::new(Mutex::new(0)) };
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["src", "flaky"], vec![Box::new(Src), Box::new(flaky.clone())]);
        // Ejecutar source y luego flaky (falla)
        engine.next_with(flow_id, &def).unwrap();
        let _ = engine.next_with(flow_id, &def); // falla, ignoramos error en test
        // Agenda retry
        let ok = engine.schedule_retry(flow_id, &def, "flaky", Some("test".into()), Some(3)).unwrap();
        assert!(ok, "Debe agendar retry");
        // Ahora next debería re-ejecutar flaky y completar
        engine.next_with(flow_id, &def).unwrap();
    let variants = engine.event_variants_for(flow_id);
    // Secuencia esperada:
    // I (init), S (src started), F (src finished), S (flaky started), X (flaky failed),
    // R (retry), S (flaky started), F (flaky finished), C (completed)
    assert_eq!(variants, vec!["I","S","F","S","X","R","S","F","C"], "Secuencia con retry R esperada");
        // Verificar que hay un sólo StepFinished para flaky (el último), y fingerprint coherente
        let evs = engine.event_store.list(flow_id);
        let fps: Vec<String> = evs.iter().filter_map(|e| {
            if let FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind {
                if step_id == "flaky" { Some(fingerprint.clone()) } else { None }
            } else { None }
        }).collect();
        assert_eq!(fps.len(), 1, "Sólo un StepFinished final por step");
        // Como el fingerprint depende de inputs/params/definition_hash y estos no cambiaron
        // entre intentos, no hay comparador directo aquí (no hubo StepFinished previo). Validamos
        // que FlowCompleted exista.
        assert!(evs.iter().any(|e| matches!(e.kind, FlowEventKind::FlowCompleted { .. })), "Debe existir FlowCompleted");
    }

    // ----------------------------------------------------------------------------------
    // TEST F7: Límite de reintentos – rechaza cuando retry_count >= max.
    // ----------------------------------------------------------------------------------
    #[test]
    fn retry_limit_respected() {
        struct AlwaysFail;
        impl StepDefinition for AlwaysFail {
            fn id(&self) -> &str { "bad" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Failure { error: CoreEngineError::Internal("e".into()) }
            }
            fn kind(&self) -> StepKind { StepKind::Transform }
        }
        struct Src;
        impl StepDefinition for Src {
            fn id(&self) -> &str { "src" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Artifact { kind: ArtifactKind::GenericJson,
                                                                  hash: String::new(),
                                                                  payload: json!({"schema_version":1}),
                                                                  metadata: None }] }
            }
            fn kind(&self) -> StepKind { StepKind::Source }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["src","bad"], vec![Box::new(Src), Box::new(AlwaysFail)]);
        engine.next_with(flow_id, &def).unwrap(); // src
        let _ = engine.next_with(flow_id, &def); // bad fails
        // Agotar dos reintentos máximo
        assert!(engine.schedule_retry(flow_id, &def, "bad", None, Some(2)).unwrap());
        // Consumir retry (fallará otra vez)
        let _ = engine.next_with(flow_id, &def);
        // Segundo retry permitido
        assert!(engine.schedule_retry(flow_id, &def, "bad", None, Some(2)).unwrap());
        let _ = engine.next_with(flow_id, &def);
        // Tercer intento de agendar debe ser rechazado
        assert!(!engine.schedule_retry(flow_id, &def, "bad", None, Some(2)).unwrap());
        assert!(engine.retries_rejected() >= 1);
    }

    // ----------------------------------------------------------------------------------
    // TEST F7: Fingerprint estable — directo vs. con retry (mismos inputs/params)
    // ----------------------------------------------------------------------------------
    #[test]
    fn fp_stable_across_retry_vs_direct_success() {
        // Source determinista
        struct Src;
        impl StepDefinition for Src {
            fn id(&self) -> &str { "src" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Artifact { kind: ArtifactKind::GenericJson,
                                                                  hash: String::new(),
                                                                  payload: json!({"v":1, "schema_version":1}),
                                                                  metadata: None }] }
            }
            fn kind(&self) -> StepKind { StepKind::Source }
        }
        // Transform estable (éxito directo)
        struct Stable;
        impl StepDefinition for Stable {
            fn id(&self) -> &str { "t" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                let _ = &ctx.input; // output determinista fijo
                StepRunResult::Success { outputs: vec![Artifact { kind: ArtifactKind::GenericJson,
                                                                  hash: String::new(),
                                                                  payload: json!({"ok":true, "schema_version":1}),
                                                                  metadata: None }] }
            }
            fn kind(&self) -> StepKind { StepKind::Transform }
        }
        // Transform flaky (falla primera vez, luego mismo output que Stable)
        use std::sync::{Arc, Mutex};
        #[derive(Clone)]
        struct Flaky { s: Arc<Mutex<u32>> }
        impl StepDefinition for Flaky {
            fn id(&self) -> &str { "t" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                let mut v = self.s.lock().unwrap();
                if *v == 0 { *v = 1; return StepRunResult::Failure { error: CoreEngineError::Internal("boom".into()) }; }
                let _ = &ctx.input;
                StepRunResult::Success { outputs: vec![Artifact { kind: ArtifactKind::GenericJson,
                                                                  hash: String::new(),
                                                                  payload: json!({"ok":true, "schema_version":1}),
                                                                  metadata: None }] }
            }
            fn kind(&self) -> StepKind { StepKind::Transform }
        }

        // Caso A: éxito directo
        let mut eng_a = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def_a = build_flow_definition(&["src","t"], vec![Box::new(Src), Box::new(Stable)]);
        let flow_a = Uuid::new_v4();
        eng_a.next_with(flow_a, &def_a).unwrap();
        eng_a.next_with(flow_a, &def_a).unwrap();
        let fp_a = eng_a.last_step_fingerprint(flow_a, "t").expect("fp_a");

        // Caso B: falla, retry, éxito
        let mut eng_b = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let flaky = Flaky { s: Arc::new(Mutex::new(0)) };
        let def_b = build_flow_definition(&["src","t"], vec![Box::new(Src), Box::new(flaky)]);
        let flow_b = Uuid::new_v4();
        eng_b.next_with(flow_b, &def_b).unwrap(); // src
        let _ = eng_b.next_with(flow_b, &def_b); // t (falla)
        assert!(eng_b.schedule_retry(flow_b, &def_b, "t", None, Some(3)).unwrap());
        eng_b.next_with(flow_b, &def_b).unwrap(); // t (éxito)
        let fp_b = eng_b.last_step_fingerprint(flow_b, "t").expect("fp_b");

        assert_eq!(fp_a, fp_b, "Fingerprint del step debe ser igual con retry");
    }

    // ----------------------------------------------------------------------------------
    // TEST 9: No se puede ejecutar después de FlowCompleted (error FlowCompleted).
    // ----------------------------------------------------------------------------------
    #[test]
    fn cannot_run_after_completion() {
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["single"], vec![Box::new(SeedStep)]); // SeedStep actúa como single
        engine.next_with(flow_id, &def).unwrap(); // ejecuta y completa
        let err = engine.next_with(flow_id, &def).unwrap_err();
        assert_eq!(err.to_string(), CoreEngineError::FlowCompleted.to_string());
    }

    // ----------------------------------------------------------------------------------
    // TEST 6: Input inválido (step requiere kind que no existe) => MissingInputs.
    // ----------------------------------------------------------------------------------
    #[test]
    fn first_step_must_be_source() {
        // Primer step no es Source => debe fallar por MissingInputs según nueva regla.
        struct TransformFirst;
        impl crate::step::StepDefinition for TransformFirst {
            fn id(&self) -> &str {
                "transform_first"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["transform_first"], vec![Box::new(TransformFirst)]);
        let err = engine.next_with(flow_id, &definition).unwrap_err();
        assert_eq!(err.to_string(), CoreEngineError::FirstStepMustBeSource.to_string());
    }

    // ----------------------------------------------------------------------------------
    // TEST 10: Cadena de steps sumaN que acumulan y generan StepSignal EVEN_SUM si
    // valor es par. sum1 (+1)=>1 impar NO señal, sum2 (+2)=>3 impar, sum3
    // (+3)=>6 par señal, sum4 (+4)=>10 par señal. Verifica orden de acumulación
    // y captura de señales.
    // ----------------------------------------------------------------------------------
    #[test]
    fn chained_increment_steps_with_even_signals() {
        #[derive(Clone, Serialize, Deserialize)]
        struct Acc {
            value: i64,
            schema_version: u32,
        }
        impl ArtifactSpec for Acc {
            const KIND: ArtifactKind = ArtifactKind::GenericJson;
        }

        // Step base generador inicial valor 0 (sin señales)
        struct Start;
        impl crate::step::StepDefinition for Start {
            fn id(&self) -> &str {
                "sum_start"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Acc { value: 0,
                                                             schema_version: 1 }.into_artifact()] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }

        // Macro para definir steps que suman N al último valor
        macro_rules! inc_step { ($name:ident, $n:expr) => {
            struct $name; impl crate::step::StepDefinition for $name {
                fn id(&self) -> &str { stringify!($name) }
                fn base_params(&self) -> serde_json::Value { json!({"inc": $n}) }
                fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                    use crate::model::TypedArtifact; let first = ctx.input.as_ref().unwrap();
                    let acc = TypedArtifact::<Acc>::decode(first).unwrap();
                    let new_v = acc.inner.value + $n;
                    let artifact = Acc { value:new_v, schema_version:1 }.into_artifact();
                    if new_v > 0 && new_v % 2 == 0 { // sólo pares mayores a 0
                        StepRunResult::SuccessWithSignals { outputs: vec![artifact], signals: vec![StepSignal { signal: "EVEN_SUM".to_string(), data: json!({"value": new_v}) }] }
                    } else {
                        StepRunResult::Success { outputs: vec![artifact] }
                    }
                }
                fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
            }
        }; }
        inc_step!(SumaStep1, 1);
        inc_step!(SumaStep2, 2);
        inc_step!(SumaStep3, 3);
        inc_step!(SumaStep4, 4);

        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps: Vec<Box<dyn StepDefinition>> = vec![Box::new(Start),
                                                       Box::new(SumaStep1),
                                                       Box::new(SumaStep2),
                                                       Box::new(SumaStep3),
                                                       Box::new(SumaStep4)];
        let ids = ["sum_start", "sumastep1", "sumastep2", "sumastep3", "sumastep4"];
        let definition = build_flow_definition(&ids, steps);
        // Ejecutar todos los steps
        for _ in 0..ids.len() {
            engine.next_with(flow_id, &definition).unwrap();
        }
        let events = engine.event_store.list(flow_id);
        // Extraer señales EVEN_SUM
        let mut signals: Vec<i64> = events.iter()
                                          .filter_map(|e| {
                                              if let FlowEventKind::StepSignal { signal, data, .. } = &e.kind {
                                                  if signal == "EVEN_SUM" {
                                                      data.get("value").and_then(|v| v.as_i64())
                                                  } else {
                                                      None
                                                  }
                                              } else {
                                                  None
                                              }
                                          })
                                          .collect();
        signals.sort();
        assert_eq!(signals, vec![6, 10], "Deben existir señales para valores pares 6 y 10");
        // Verificar valor final 10 en último StepFinished
        let final_value = events.iter().rev().find_map(|e| {
                                                 if let FlowEventKind::StepFinished { step_id, .. } = &e.kind {
                                                     if step_id == "sumastep4" {
                                                         // Recuperar artifact correspondiente
                                                         // buscamos hash del output en ese evento y lo resolvemos del
                                                         // artifact_store
                                                         if let FlowEventKind::StepFinished { outputs, .. } = &e.kind {
                                                             outputs.first().cloned()
                                                         } else {
                                                             None
                                                         }
                                                     } else {
                                                         None
                                                     }
                                                 } else {
                                                     None
                                                 }
                                             });
        // Necesitamos leer artifact_store interno -> acceso directo (no ideal en prod,
        // aceptable test).
        if let Some(h) = final_value {
            let art = engine.artifact_store.get(&h).unwrap();
            assert_eq!(art.payload.get("value").unwrap().as_i64().unwrap(), 10);
        }
    }

    // ----------------------------------------------------------------------------------
    // TEST 11: Emite una señal personalizada PRINT_HELLO y el test imprime "hola"
    // al verla. Demuestra cómo un consumidor (el test) puede reaccionar a
    // StepSignal sin lógica en el engine.
    // ----------------------------------------------------------------------------------
    #[test]
    fn signal_triggers_side_effect_print_hello() {
        #[derive(Clone, Serialize, Deserialize)]
        struct Dummy {
            v: i32,
            schema_version: u32,
        }
        impl ArtifactSpec for Dummy {
            const KIND: ArtifactKind = ArtifactKind::GenericJson;
        }
        struct HelloSignalStep;
        impl crate::step::StepDefinition for HelloSignalStep {
            fn id(&self) -> &str {
                "hello_signal"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                let art = Dummy { v: 1, schema_version: 1 }.into_artifact();
                StepRunResult::SuccessWithSignals { outputs: vec![art],
                                                    signals: vec![StepSignal { signal: "PRINT_HELLO".to_string(),
                                                                               data: json!({}) }] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["hello_signal"], vec![Box::new(HelloSignalStep)]);
        engine.next_with(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
        let mut found = false;
        for e in events {
            if let FlowEventKind::StepSignal { signal, .. } = e.kind {
                if signal == "PRINT_HELLO" {
                    println!("hola");
                    found = true;
                }
            }
        }
        assert!(found, "Debe haberse emitido la señal PRINT_HELLO");
    }

    // ----------------------------------------------------------------------------------
    // TEST 7: canonical_json ordering produce mismo hash para objetos con claves
    // invertidas.
    // ----------------------------------------------------------------------------------
    #[test]
    fn canonical_json_ordering() {
        use crate::hashing::{hash_value, to_canonical_json};
        let a = json!({"b":2,"a":1});
        let b = json!({"a":1,"b":2});
        assert_eq!(to_canonical_json(&a), to_canonical_json(&b));
        assert_eq!(hash_value(&a), hash_value(&b));
    }

    // ----------------------------------------------------------------------------------
    // TEST 8 (G5): 20 iteraciones de canonical_json sobre construcciones con orden
    // variable. Verifica estabilidad absoluta del hash.
    // ----------------------------------------------------------------------------------
    #[test]
    fn canonical_json_repetition_20() {
        use crate::hashing::{hash_value, to_canonical_json};
        let base = json!({"k1":1, "k2": {"z": true, "a": false}, "array": [3,2,1]});
        let expected_canonical = to_canonical_json(&base);
        let expected_hash = hash_value(&base);
        // Generamos permutaciones deterministas (rotaciones) de las claves recreando
        // Value manualmente.
        let orderings = vec![vec!["k1", "k2", "array"],
                             vec!["k2", "array", "k1"],
                             vec!["array", "k1", "k2"],
                             vec!["array", "k2", "k1"],
                             vec!["k2", "k1", "array"],
                             vec!["k1", "array", "k2"]];
        for i in 0..20 {
            let ord = &orderings[i % orderings.len()];
            let mut map = serde_json::Map::new();
            for k in ord {
                match *k {
                    "k1" => {
                        map.insert((*k).to_string(), json!(1));
                    }
                    "k2" => {
                        map.insert((*k).to_string(), json!({"z": true, "a": false}));
                    }
                    "array" => {
                        map.insert((*k).to_string(), json!([3, 2, 1]));
                    }
                    _ => unreachable!(),
                }
            }
            let v = serde_json::Value::Object(map);
            assert_eq!(to_canonical_json(&v),
                       expected_canonical,
                       "Canonical JSON mismatch iteration {i}");
            assert_eq!(hash_value(&v), expected_hash, "Hash mismatch iteration {i}");
        }
    }

    // ----------------------------------------------------------------------------------
    // TEST 12: Flujo de 2 steps: StepSeven produce número 7; StepReemite lo recibe
    // (typed) y re-emite nuevamente el 7 y además un segundo artifact con
    // mensaje "hola como estas". Verifica paso de artifacts y contenido
    // múltiple.
    // ----------------------------------------------------------------------------------
    #[test]
    fn two_step_number_and_message_flow() {
        #[derive(Clone, Serialize, Deserialize)]
        struct Numero {
            value: i64,
            schema_version: u32,
        }
        #[derive(Clone, Serialize, Deserialize)]
        struct Mensaje {
            msg: String,
            schema_version: u32,
        }
        use crate::model::TypedArtifact;
        impl ArtifactSpec for Numero {
            const KIND: ArtifactKind = ArtifactKind::GenericJson;
        }
        impl ArtifactSpec for Mensaje {
            const KIND: ArtifactKind = ArtifactKind::GenericJson;
        }

        struct StepSeven;
        impl StepDefinition for StepSeven {
            fn id(&self) -> &str {
                "step_seven"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Numero { value: 7,
                                                                schema_version: 1 }.into_artifact()] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }

        struct StepReemite;
        impl StepDefinition for StepReemite {
            fn id(&self) -> &str {
                "step_reemite"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                let num_art = ctx.input.as_ref().unwrap();
                let n = TypedArtifact::<Numero>::decode(num_art).unwrap();
                assert_eq!(n.inner.value, 7, "Debe recibir 7");
                let a1 = Numero { value: n.inner.value,
                                  schema_version: 1 }.into_artifact();
                let a2 = Mensaje { msg: "hola como estas".to_string(),
                                   schema_version: 1 }.into_artifact();
                StepRunResult::Success { outputs: vec![a1, a2] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }

        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["step_seven", "step_reemite"],
                                               vec![Box::new(StepSeven), Box::new(StepReemite)]);
        engine.next_with(flow_id, &definition).unwrap(); // step 0
        engine.next_with(flow_id, &definition).unwrap(); // step 1
        let events = engine.event_store.list(flow_id);
        // localizar StepFinished del segundo step
        let finished = events.iter()
                             .find(|e| match &e.kind {
                                 FlowEventKind::StepFinished { step_id, .. } if step_id == "step_reemite" => true,
                                 _ => false,
                             })
                             .unwrap();
        let output_hashes = if let FlowEventKind::StepFinished { outputs, .. } = &finished.kind {
            outputs.clone()
        } else {
            vec![]
        };
        assert_eq!(output_hashes.len(), 2, "Debe producir dos artifacts (numero y mensaje)");
        // decodificar ambos
        let mut have_number7 = false;
        let mut have_message = false;
        for h in output_hashes {
            let art = engine.artifact_store.get(&h).unwrap();
            if art.payload.get("value").and_then(|v| v.as_i64()) == Some(7) {
                have_number7 = true;
            }
            if art.payload.get("msg").and_then(|v| v.as_str()) == Some("hola como estas") {
                have_message = true;
            }
        }
        assert!(have_number7 && have_message, "Deben existir el 7 y el mensaje");
    }

    // ----------------------------------------------------------------------------------
    // TEST 13: Flujo con señal: StepSeven produce 7; StepDetect emite StepSignal
    // HAY_UN_7 y en vez de reenviar 7 produce un 9. StepConsume recibe 9 y
    // valida. Verifica paso de artifacts transformados y emisión de señal.
    // ----------------------------------------------------------------------------------
    #[test]
    fn signal_and_transform_number_flow() {
        #[derive(Clone, Serialize, Deserialize)]
        struct Numero {
            value: i64,
            schema_version: u32,
        }
        use crate::model::TypedArtifact;
        impl ArtifactSpec for Numero {
            const KIND: ArtifactKind = ArtifactKind::GenericJson;
        }

        struct StepSeven;
        impl StepDefinition for StepSeven {
            fn id(&self) -> &str {
                "step_seven2"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Numero { value: 7,
                                                                schema_version: 1 }.into_artifact()] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }

        struct StepDetect;
        impl StepDefinition for StepDetect {
            fn id(&self) -> &str {
                "step_detect"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                let first = ctx.input.as_ref().unwrap();
                let num = TypedArtifact::<Numero>::decode(first).unwrap();
                if num.inner.value == 7 {
                    let out = Numero { value: 9,
                                       schema_version: 1 }.into_artifact();
                    StepRunResult::SuccessWithSignals { outputs: vec![out],
                                                        signals: vec![StepSignal { signal: "HAY_UN_7".to_string(),
                                                                                   data: json!({"original":7}) }] }
                } else {
                    StepRunResult::Success { outputs: vec![Numero { value: num.inner.value,
                                                                    schema_version: 1 }.into_artifact()] }
                }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }

        struct StepConsume;
        impl StepDefinition for StepConsume {
            fn id(&self) -> &str {
                "step_consume"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({})
            }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                // Puede haber múltiples artifacts previos (7 original + 9 transformado).
                // Tomamos el último (más reciente).
                let latest = ctx.input.as_ref().unwrap();
                let num = TypedArtifact::<Numero>::decode(latest).unwrap();
                assert_eq!(num.inner.value, 9, "Debe recibir 9 transformado");
                StepRunResult::Success { outputs: vec![latest.clone()] } // re-emite artifact transformado
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["step_seven2", "step_detect", "step_consume"],
                                               vec![Box::new(StepSeven), Box::new(StepDetect), Box::new(StepConsume)]);
        engine.next_with(flow_id, &definition).unwrap(); // produce 7
        engine.next_with(flow_id, &definition).unwrap(); // detect -> señal + 9
        engine.next_with(flow_id, &definition).unwrap(); // consume 9
        let events = engine.event_store.list(flow_id);
        assert!(events.iter()
                      .any(|e| matches!(e.kind, FlowEventKind::StepSignal { ref signal, .. } if signal=="HAY_UN_7")),
                "Debe emitirse señal HAY_UN_7");
        let last_finished = events.iter()
                                  .rev()
                                  .find(|e| match &e.kind {
                                      FlowEventKind::StepFinished { step_id, .. } if step_id == "step_consume" => true,
                                      _ => false,
                                  })
                                  .expect("missing consume finish");
        if let FlowEventKind::StepFinished { outputs, .. } = &last_finished.kind {
            assert_eq!(outputs.len(), 1);
        }
    }

    // ----------------------------------------------------------------------------------
    // TEST 14: definition_hash depende sólo de ids (orden) – snapshot simple.
    // ----------------------------------------------------------------------------------
    #[test]
    fn definition_hash_only_ids() {
        struct A;
        impl StepDefinition for A {
            fn id(&self) -> &str {
                "a"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({"x":1})
            }
            fn run(&self, _: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }
        struct B;
        impl StepDefinition for B {
            fn id(&self) -> &str {
                "b"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({"y":2})
            }
            fn run(&self, _: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }
        let def1 = build_flow_definition(&["a", "b"], vec![Box::new(A), Box::new(B)]);
        // Cambiamos parámetros internos pero mismo orden de ids -> mismo hash.
        struct A2;
        impl StepDefinition for A2 {
            fn id(&self) -> &str {
                "a"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({"x":999})
            }
            fn run(&self, _: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Source
            }
        }
        struct B2;
        impl StepDefinition for B2 {
            fn id(&self) -> &str {
                "b"
            }
            fn base_params(&self) -> serde_json::Value {
                json!({"y":0})
            }
            fn run(&self, _: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![] }
            }
            fn kind(&self) -> crate::step::StepKind {
                crate::step::StepKind::Transform
            }
        }
        let def2 = build_flow_definition(&["a", "b"], vec![Box::new(A2), Box::new(B2)]);
        assert_eq!(def1.definition_hash, def2.definition_hash,
                   "definition_hash debe depender solo de ids");
        // Cambiar orden ids cambia hash.
        let def_swapped = build_flow_definition(&["b", "a"], vec![Box::new(B2), Box::new(A2)]);
        assert_ne!(def1.definition_hash, def_swapped.definition_hash,
                   "Cambiar orden ids debe cambiar hash");
    }

    // ----------------------------------------------------------------------------------
    // TEST 15: Flow fingerprint agregado determinista entre runs idénticos.
    // ----------------------------------------------------------------------------------
    #[test]
    fn aggregated_flow_fingerprint_deterministic() {
        let flow_id = Uuid::new_v4();
        let mut e1 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["seed", "sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        e1.next_with(flow_id, &def).unwrap();
        e1.next_with(flow_id, &def).unwrap();
        let fp1 = e1.test_compute_flow_fingerprint(flow_id);
        let mut e2 = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def2 = build_flow_definition(&["seed", "sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        e2.next_with(flow_id, &def2).unwrap();
        e2.next_with(flow_id, &def2).unwrap();
        let fp2 = e2.test_compute_flow_fingerprint(flow_id);
        assert_eq!(fp1, fp2, "Flow fingerprint agregado debe ser estable");
    }

    // ----------------------------------------------------------------------------------
    // TEST F6: Traducción de StepSignal reservada a PropertyPreferenceAssigned.
    // Verifica que el engine emite P antes de F y no deja la señal genérica.
    // ----------------------------------------------------------------------------------
    #[test]
    fn reserved_signal_translates_to_policy_event() {
        use serde_json::json;
        // Step fuente que emite la señal reservada con payload válido
        struct PolicySource;
        impl crate::step::StepDefinition for PolicySource {
            fn id(&self) -> &str { "policy_src" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                let art = serde_json::json!({"dummy":true, "schema_version":1});
                // Artifact genérico
                let artifact = crate::model::Artifact { kind: crate::model::ArtifactKind::GenericJson,
                                                        hash: String::new(),
                                                        payload: art,
                                                        metadata: None };
                let data = json!({
                    "property_key": "inchikey:XYZ|prop:foo",
                    "policy_id": "max_score",
                    "params_hash": "abcd1234",
                    "rationale": {"score": 0.99}
                });
                StepRunResult::SuccessWithSignals { outputs: vec![artifact],
                                                    signals: vec![StepSignal { signal: "PROPERTY_PREFERENCE_ASSIGNED".into(),
                                                                               data }] }
            }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["policy_src"], vec![Box::new(PolicySource)]);
        engine.next_with(flow_id, &def).unwrap();
        // Debe completar el flow de un solo step
        let variants = engine.event_variants_for(flow_id);
        assert_eq!(variants, vec!["I", "S", "P", "F", "C"], "Secuencia debe incluir P antes de F");
        // No debe existir StepSignal con ese nombre; en su lugar, un evento P tipado
        let events = engine.event_store.list(flow_id);
        assert!(events.iter().any(|e| matches!(e.kind, FlowEventKind::PropertyPreferenceAssigned{..})),
                "Debe existir PropertyPreferenceAssigned");
        assert!(!events.iter().any(|e| matches!(e.kind, FlowEventKind::StepSignal{ ref signal, .. } if signal=="PROPERTY_PREFERENCE_ASSIGNED")),
                "No debe quedar StepSignal genérica para la señal reservada");
    }
}
