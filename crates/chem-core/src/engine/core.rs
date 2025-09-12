//! Core FlowEngine implementation

use crate::engine::EngineBuilderInit;
use crate::errors::CoreEngineError;
use crate::event::{EventStore, FlowEventKind};
use crate::hashing::hash_value;
use crate::model::{Artifact, ExecutionContext};
use crate::repo::{FlowDefinition, FlowRepository};
use crate::StepDefinition;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// Motor de ejecución de flujos deterministas
///
/// Responsable de orquestar la ejecución de pasos, mantener el estado
/// interno y garantizar el determinismo mediante fingerprints
#[derive(Debug)]
pub struct FlowEngine<E, R>
    where E: EventStore,
          R: FlowRepository
{
    event_store: E,
    repository: R,
    artifact_store: HashMap<String, Artifact>,
    injectors: Vec<Box<dyn crate::injection::ParamInjector>>,
    default_flow_id: Option<Uuid>,
    default_definition: Option<FlowDefinition>,
}

impl<E, R> FlowEngine<E, R>
    where E: EventStore,
          R: FlowRepository
{
    /// Crea un nuevo builder para configurar el engine
    #[inline]
    pub fn builder(event_store: E, repository: R) -> EngineBuilderInit<E, R> {
        EngineBuilderInit { event_store, repository }
    }

    /// Crea un nuevo engine con stores en memoria
    #[inline]
    pub fn new() -> EngineBuilderInit<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository> {
        EngineBuilderInit { event_store: crate::event::InMemoryEventStore::default(),
                            repository: crate::repo::InMemoryFlowRepository::new() }
    }

    /// Crea un nuevo motor con los stores proporcionados
    pub fn new_with_stores(event_store: E, repository: R) -> Self {
        Self { event_store,
               repository,
               artifact_store: HashMap::new(),
               injectors: Vec::new(),
               default_flow_id: None,
               default_definition: None }
    }

    /// Añade un inyector de parámetros
    pub fn add_injector(&mut self, injector: Box<dyn crate::injection::ParamInjector>) {
        self.injectors.push(injector);
    }

    /// Recupera un artifact por su hash
    pub fn get_artifact(&self, hash: &str) -> Option<&Artifact> {
        self.artifact_store.get(hash)
    }

    /// Almacena un artifact en la cache local
    pub fn store_artifact(&mut self, artifact: Artifact) {
        self.artifact_store.insert(artifact.hash.clone(), artifact);
    }

    /// Ensure a FlowInitialized event exists and return the current events
    /// for the flow (including the possibly newly appended FlowInitialized).
    fn load_or_init(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Vec<crate::event::FlowEvent> {
        let mut events = self.event_store.list(flow_id);
        let has_init = events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. }));
        if !has_init {
            let ev = self.event_store
                         .append_kind(flow_id,
                                      FlowEventKind::FlowInitialized { definition_hash: definition.definition_hash.clone(),
                                                                       step_count: definition.len() });
            events.push(ev);
        }
        self.default_flow_id = Some(flow_id);
        events
    }

    /// Define/genera un `flow_id` por defecto si no existe aún y lo retorna.
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

    /// Obtiene el `flow_id` por defecto si está configurado.
    pub fn default_flow_id(&self) -> Option<Uuid> {
        self.default_flow_id
    }

    fn hash_and_store_outputs(&mut self, outputs: &mut [Artifact]) -> Vec<String> {
        let mut hashes: Vec<String> = Vec::with_capacity(outputs.len());
        for o in outputs.iter_mut() {
            let h = hash_value(&o.payload);
            o.hash = h.clone();
            self.store_artifact(o.clone());
            hashes.push(h);
        }
        hashes
    }

    /// Ejecuta el flujo completo y retorna el ID del flujo ejecutado
    ///
    /// # Ejemplo
    /// ```
    /// let flow_id = engine.run()?;
    /// ```
    pub fn run(&mut self) -> Result<Uuid, CoreEngineError> {
        self.run_to_completion()
    }

    /// Avanza un paso en la ejecución del flujo
    pub fn step(&mut self) -> Result<(), CoreEngineError> {
        self.next()
    }

    /// Configura la definición por defecto del flujo
    pub fn set_default_definition(&mut self, definition: FlowDefinition) {
        self.default_definition = Some(definition);
    }

    /// Obtiene los eventos del flujo actual
    pub fn get_events(&self) -> Option<Vec<crate::event::FlowEvent>> {
        self.events()
    }

    /// Ejecuta el flujo completo usando la definición por defecto
    pub fn run_to_completion(&mut self) -> Result<Uuid, CoreEngineError> {
        let flow_id = self.ensure_default_flow_id();
        let def = self.default_definition
                      .take()
                      .ok_or_else(|| CoreEngineError::Internal("no default definition configured".into()))?;

        let result = self.run_flow_to_completion(flow_id, &def);
        self.default_definition = Some(def);
        result
    }

    /// Ejecuta un flujo específico hasta su finalización
    pub fn run_flow_to_completion(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<Uuid, CoreEngineError> {
        loop {
            match self.next_with(flow_id, definition) {
                Ok(()) => continue,
                Err(CoreEngineError::FlowCompleted) => return Ok(flow_id),
                Err(e) => return Err(e),
            }
        }
    }

    /// Ejecuta un paso específico del flujo
    pub(crate) fn next_with(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
        let events = self.load_or_init(flow_id, definition);
        let instance = self.repository.load(flow_id, &events, definition);

        if instance.completed {
            return Err(CoreEngineError::FlowCompleted);
        }

        let cursor = instance.cursor;
        if cursor >= definition.len() {
            return Err(CoreEngineError::FlowCompleted);
        }

        let step_def = &definition.steps[cursor];
        let input = if cursor == 0 {
            None
        } else {
            instance.steps
                    .get(cursor - 1)
                    .and_then(|s| s.outputs.get(0))
                    .and_then(|h| self.artifact_store.get(h).cloned())
        };

        let ctx = ExecutionContext { input,
                                     params: step_def.base_params() };

        let _started = self.event_store.append_kind(flow_id,
                                                    FlowEventKind::StepStarted { step_index: cursor,
                                                                                 step_id: step_def.id().to_string() });

        let run_res = step_def.run(&ctx);

        match run_res {
            crate::step::StepRunResult::Success { outputs } => {
                self.handle_step_success(flow_id, cursor, step_def, outputs, definition)
            }
            crate::step::StepRunResult::SuccessWithSignals { outputs, signals } => {
                self.handle_step_success_with_signals(flow_id, cursor, step_def, outputs, signals, definition)
            }
            crate::step::StepRunResult::Failure { error } => self.handle_step_failure(flow_id, cursor, step_def, error),
        }
    }

    fn handle_step_success(&mut self,
                           flow_id: Uuid,
                           cursor: usize,
                           step_def: &dyn StepDefinition,
                           mut outputs: Vec<Artifact>,
                           definition: &FlowDefinition)
                           -> Result<(), CoreEngineError> {
        let output_hashes = self.hash_and_store_outputs(&mut outputs);
        let fp = self.calculate_step_fingerprint(cursor, step_def, &output_hashes, definition);

        let _finished = self.event_store.append_kind(flow_id,
                                                     FlowEventKind::StepFinished { step_index: cursor,
                                                                                   step_id: step_def.id().to_string(),
                                                                                   outputs: output_hashes.clone(),
                                                                                   fingerprint: fp.clone() });

        if cursor + 1 == definition.len() {
            self.complete_flow(flow_id, definition);
        }

        Ok(())
    }

    fn handle_step_success_with_signals(&mut self,
                                        flow_id: Uuid,
                                        cursor: usize,
                                        step_def: &dyn StepDefinition,
                                        mut outputs: Vec<Artifact>,
                                        signals: Vec<crate::step::StepSignal>,
                                        definition: &FlowDefinition)
                                        -> Result<(), CoreEngineError> {
        let output_hashes = self.hash_and_store_outputs(&mut outputs);

        for s in signals {
            let _ = self.event_store.append_kind(flow_id,
                                                 FlowEventKind::StepSignal { step_index: cursor,
                                                                             step_id: step_def.id().to_string(),
                                                                             signal: s.signal,
                                                                             data: s.data });
        }

        let fp = self.calculate_step_fingerprint(cursor, step_def, &output_hashes, definition);

        let _finished = self.event_store.append_kind(flow_id,
                                                     FlowEventKind::StepFinished { step_index: cursor,
                                                                                   step_id: step_def.id().to_string(),
                                                                                   outputs: output_hashes.clone(),
                                                                                   fingerprint: fp.clone() });

        if cursor + 1 == definition.len() {
            self.complete_flow(flow_id, definition);
        }

        Ok(())
    }

    fn handle_step_failure(&mut self,
                           flow_id: Uuid,
                           cursor: usize,
                           step_def: &dyn StepDefinition,
                           error: CoreEngineError)
                           -> Result<(), CoreEngineError> {
        let fp_json = json!({
            "engine_version": crate::constants::ENGINE_VERSION,
            "definition_hash": step_def.definition_hash(),
            "step_index": cursor,
            "params": step_def.base_params()
        });
        let fp = hash_value(&fp_json);

        let _ = self.event_store.append_kind(flow_id,
                                             FlowEventKind::StepFailed { step_index: cursor,
                                                                         step_id: step_def.id().to_string(),
                                                                         error: error,
                                                                         fingerprint: fp });

        Err(error)
    }

    fn calculate_step_fingerprint(&self,
                                  cursor: usize,
                                  step_def: &dyn StepDefinition,
                                  output_hashes: &[String],
                                  definition: &FlowDefinition)
                                  -> String {
        let fp_json = json!({
            "engine_version": crate::constants::ENGINE_VERSION,
            "definition_hash": definition.definition_hash,
            "step_index": cursor,
            "output_hashes": output_hashes,
            "params": step_def.base_params()
        });
        hash_value(&fp_json)
    }

    fn complete_flow(&mut self, flow_id: Uuid, definition: &FlowDefinition) {
        let events = self.event_store.list(flow_id);
        let step_fps: Vec<String> = events.iter()
                                          .filter_map(|e| match &e.kind {
                                              FlowEventKind::StepFinished { fingerprint, .. } => Some(fingerprint.clone()),
                                              _ => None,
                                          })
                                          .collect();

        let flow_fp = hash_value(&json!({
                                     "engine_version": crate::constants::ENGINE_VERSION,
                                     "definition_hash": definition.definition_hash,
                                     "step_fingerprints": step_fps
                                 }));

        let _ = self.event_store
                    .append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint: flow_fp });
    }

    /// Avanza un paso en el flujo por defecto
    pub fn next(&mut self) -> Result<(), CoreEngineError> {
        let flow_id = self.ensure_default_flow_id();
        let def = self.default_definition
                      .as_ref()
                      .ok_or_else(|| CoreEngineError::Internal("no default definition configured".into()))?
                      .clone();

        self.next_with(flow_id, &def)
    }

    /// Lista eventos del flujo por defecto
    pub fn events(&self) -> Option<Vec<crate::event::FlowEvent>> {
        self.default_flow_id.map(|fid| self.event_store.list(fid))
    }

    /// Variante compacta de eventos para el flujo por defecto
    pub fn event_variants(&self) -> Option<Vec<&'static str>> {
        self.events().map(|events| {
                         events.iter()
                               .map(|e| match e.kind {
                                   FlowEventKind::FlowInitialized { .. } => "I",
                                   FlowEventKind::StepStarted { .. } => "S",
                                   FlowEventKind::StepFinished { .. } => "F",
                                   FlowEventKind::StepFailed { .. } => "X",
                                   FlowEventKind::StepSignal { .. } => "G",
                                   FlowEventKind::PropertyPreferenceAssigned { .. } => "P",
                                   FlowEventKind::RetryScheduled { .. } => "R",
                                   FlowEventKind::BranchCreated { .. } => "B",
                                   FlowEventKind::UserInteractionRequested { .. } => "U",
                                   FlowEventKind::UserInteractionProvided { .. } => "V",
                                   FlowEventKind::FlowCompleted { .. } => "C",
                               })
                               .collect()
                     })
    }

    /// Fingerprint del flujo por defecto si está presente
    pub fn flow_fingerprint(&self) -> Option<String> {
        let evs = self.events()?;
        evs.iter().rev().find_map(|e| match &e.kind {
                            FlowEventKind::FlowCompleted { flow_fingerprint } => Some(flow_fingerprint.clone()),
                            _ => None,
                        })
    }
}

impl Default for FlowEngine<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository> {
    fn default() -> Self {
        Self::new_with_stores(crate::event::InMemoryEventStore::default(),
                              crate::repo::InMemoryFlowRepository::new())
    }
}
