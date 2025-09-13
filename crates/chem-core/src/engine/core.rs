//! Core FlowEngine implementation.
//!
//! Responsable de orquestar la ejecución de pasos, mantener el estado interno
//! (artifacts, definición por defecto, etc.) y garantizar determinismo mediante
//! fingerprints por paso y del flujo completo.

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

/// Motor de ejecución de flujos deterministas.
#[derive(Debug)]
pub struct FlowEngine<E, R>
where
    E: EventStore,
    R: FlowRepository,
{
    /// Store de eventos que usará el engine.
    pub event_store: E,
    repository: R,

    /// Cache local de artifacts indexados por su hash.
    artifact_store: HashMap<String, Artifact>,

    /// Inyectores de parámetros (no usados en este archivo, pero parte del API).
    pub injectors: Vec<Box<dyn crate::injection::ParamInjector>>,

    /// Flow id por defecto que usan los métodos sin argumentos.
    pub default_flow_id: Option<Uuid>,

    /// Definición por defecto (opcional) que permite ejecutar `run()` y `next()`.
    pub default_definition: Option<FlowDefinition>,
}

impl<E, R> FlowEngine<E, R>
where
    E: EventStore,
    R: FlowRepository,
{
    // -- Constructores / helpers públicos -------------------------------------------------

    /// Crea un builder (`EngineBuilderInit`) para configurar stores y pasos.
    #[inline]
    pub fn builder(event_store: E, repository: R) -> EngineBuilderInit<E, R> {
        EngineBuilderInit { event_store, repository }
    }

    /// Crea un builder con stores en memoira (útil para tests y ejemplos).
    #[inline]
    pub fn new() -> EngineBuilderInit<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository> {
        EngineBuilderInit {
            event_store: crate::event::InMemoryEventStore::default(),
            repository: crate::repo::InMemoryFlowRepository::new(),
        }
    }

    /// Construye un `FlowEngine` con stores proporcionadas.
    pub fn new_with_stores(event_store: E, repository: R) -> Self {
        Self {
            event_store,
            repository,
            artifact_store: HashMap::new(),
            injectors: Vec::new(),
            default_flow_id: None,
            default_definition: None,
        }
    }

    /// Construye un `FlowEngine` con stores y definición por defecto.
    pub fn new_with_definition(event_store: E, repository: R, definition: FlowDefinition) -> Self {
        Self {
            event_store,
            repository,
            artifact_store: HashMap::new(),
            injectors: Vec::new(),
            default_flow_id: None,
            default_definition: Some(definition),
        }
    }

    // -- Public API: mutation / queries ---------------------------------------------------

    /// Añade un inyector de parámetros al engine.
    pub fn add_injector(&mut self, injector: Box<dyn crate::injection::ParamInjector>) {
        self.injectors.push(injector);
    }

    /// Recupera un artifact por su hash desde la cache local.
    pub fn get_artifact(&self, hash: &str) -> Option<&Artifact> {
        self.artifact_store.get(hash)
    }

    /// Almacena un artifact en la cache local (reemplaza si ya existe).
    pub fn store_artifact(&mut self, artifact: Artifact) {
        self.artifact_store.insert(artifact.hash.clone(), artifact);
    }

    /// Asegura que existe un `FlowInitialized` y devuelve la lista de eventos
    /// actuales del flujo (incluyendo la posible inserción de `FlowInitialized`).
    fn load_or_init(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Vec<crate::event::FlowEvent> {
        let mut events = self.event_store.list(flow_id);
        let has_init = events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. }));
        if !has_init {
            let ev = self.event_store.append_kind(
                flow_id,
                FlowEventKind::FlowInitialized {
                    definition_hash: definition.definition_hash.clone(),
                    step_count: definition.len(),
                },
            );
            events.push(ev);
        }
        self.default_flow_id = Some(flow_id);
        events
    }

    /// Genera/obtiene el `flow_id` por defecto si no existe y lo retorna.
    pub fn ensure_default_flow_id(&mut self) -> Uuid {
        if self.default_flow_id.is_none() {
            self.default_flow_id = Some(Uuid::new_v4());
        }
        self.default_flow_id.unwrap()
    }

    /// Establece explícitamente el `flow_id` por defecto.
    pub fn set_default_flow_id(&mut self, flow_id: Uuid) {
        self.default_flow_id = Some(flow_id);
    }

    /// Devuelve el `flow_id` por defecto si está configurado.
    pub fn default_flow_id(&self) -> Option<Uuid> {
        self.default_flow_id
    }

    /// Crea una rama a partir de una lista completa de pasos (owned).
    ///
    /// Este helper construye una `FlowDefinition` a partir de los `steps`
    /// proporcionados y delega en `branch_builder` para crear/ejecutar la rama
    /// en memoria. Se devuelve el `Uuid` de la rama creada.
     pub fn create_branch_from_steps(&mut self, parent_flow_id: Uuid, steps: Vec<Box<dyn StepDefinition>>, from_step_id: &str) -> Result<Uuid, CoreEngineError> {
        // Construir la definición a partir de los pasos proporcionados
        let def = crate::repo::build_flow_definition_auto(steps);

        // Delegar en el builder de ramas existente (espera una FlowDefinition owned)
        let mut builder = self.branch_builder(parent_flow_id, def, from_step_id, None)?;

        // Ejecutar la rama hasta completar y devolver su id.
        builder.run_to_completion()
    }

    /// Variante: crear una rama a partir de pasos indicando el índice del step
    /// en lugar del `step_id`. Esto es útil cuando la llamada es puramente
    /// posicional (por ejemplo "desde el paso 1").
    pub fn create_branch_from_steps_at_index(
        &mut self,
        parent_flow_id: Uuid,
        steps: Vec<Box<dyn StepDefinition>>,
        from_step_index: usize,
    ) -> Result<Uuid, CoreEngineError> {
        let def = crate::repo::build_flow_definition_auto(steps);
        let mut builder = self.branch_builder_by_index(parent_flow_id, def, from_step_index, None)?;
        builder.run_to_completion()
    }

    /// Conveniencia: crear y ejecutar una rama a partir de un único `StepDefinition`.
    ///
    /// Esto es útil cuando queremos añadir rápidamente un step nuevo y lanzar
    /// una rama que lo contenga sin construir manualmente una `FlowDefinition`.
    pub fn create_branch_with_step(
        &mut self,
        parent_flow_id: Uuid,
        step: Box<dyn StepDefinition>,
        from_step_id: &str,
    ) -> Result<Uuid, CoreEngineError> {
        let steps = vec![step];
        self.create_branch_from_steps(parent_flow_id, steps, from_step_id)
    }

    /// Conveniencia: crear y ejecutar una rama a partir de un único paso,
    /// indicando la posición del punto de bifurcación.
    pub fn create_branch_with_step_at_index(
        &mut self,
        parent_flow_id: Uuid,
        step: Box<dyn StepDefinition>,
        from_step_index: usize,
    ) -> Result<Uuid, CoreEngineError> {
        let steps = vec![step];
        self.create_branch_from_steps_at_index(parent_flow_id, steps, from_step_index)
    }

    /// Devuelve una referencia a la definición por defecto si está configurada.
    pub fn default_definition_ref(&self) -> Option<&FlowDefinition> {
        self.default_definition.as_ref()
    }

    /// Hashea y guarda todos los outputs devueltos por un paso.
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

    // -- High level execution helpers ----------------------------------------------------

    /// Ejecuta el flujo completo usando la definición por defecto y devuelve el `flow_id`.
    pub fn run(&mut self) -> Result<Uuid, CoreEngineError> {
        self.run_to_completion()
    }

    /// Avanza un solo paso en el flujo por defecto.
    pub fn step(&mut self) -> Result<(), CoreEngineError> {
        self.next()
    }

    /// Establece la definición por defecto del flujo.
    pub fn set_default_definition(&mut self, definition: FlowDefinition) {
        self.default_definition = Some(definition);
    }

    /// Devuelve los eventos del flujo por defecto, si existe `default_flow_id`.
    pub fn get_events(&self) -> Option<Vec<crate::event::FlowEvent>> {
        self.events()
    }

    /// Ejecuta el flujo por defecto hasta su finalización.
    pub fn run_to_completion(&mut self) -> Result<Uuid, CoreEngineError> {
        let flow_id = self.ensure_default_flow_id();
        let def = self
            .default_definition
            .take()
            .ok_or_else(|| CoreEngineError::Internal("no default definition configured".into()))?;

        let result = self.run_flow_to_completion(flow_id, &def);
        // Restaurar la definición por defecto puesto que la tomamos temporalmente
        self.default_definition = Some(def);
        result
    }

    /// Ejecuta un flujo específico hasta su finalización.
    pub fn run_flow_to_completion(
        &mut self,
        flow_id: Uuid,
        definition: &FlowDefinition,
    ) -> Result<Uuid, CoreEngineError> {
        loop {
            match self.next_with(flow_id, definition) {
                Ok(()) => continue,
                Err(CoreEngineError::FlowCompleted) => return Ok(flow_id),
                Err(e) => return Err(e),
            }
        }
    }

    /// Ejecuta un paso del flujo especificado por `flow_id` usando `definition`.
    ///
    /// Se encarga de cargar/crear el evento inicial, construir el contexto de
    /// ejecución y despachar al `StepDefinition` correspondiente.
    pub fn next_with(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
        let events = self.load_or_init(flow_id, definition);
        let instance = self.repository.load(flow_id, &events, definition);

        if instance.completed {
            return Err(CoreEngineError::FlowCompleted);
        }

        let cursor = instance.cursor;
        if cursor >= definition.len() {
            return Err(CoreEngineError::FlowCompleted);
        }

        // Resolve input artifact for this step (if any)
        let step_def = &definition.steps[cursor];
        let input = if cursor == 0 {
            None
        } else {
            instance
                .steps
                .get(cursor - 1)
                .and_then(|s| s.outputs.get(0))
                .and_then(|h| self.artifact_store.get(h).cloned())
        };

        let ctx = ExecutionContext {
            input,
            params: step_def.base_params(),
        };

        let _started = self.event_store.append_kind(
            flow_id,
            FlowEventKind::StepStarted {
                step_index: cursor,
                step_id: step_def.id().to_string(),
            },
        );

        let run_res = step_def.run(&ctx);

        match run_res {
            crate::step::StepRunResult::Success { outputs } => {
                self.handle_step_success(flow_id, cursor, step_def, outputs, definition)
            }
            crate::step::StepRunResult::SuccessWithSignals { outputs, signals } => {
                self.handle_step_success_with_signals(flow_id, cursor, step_def, outputs, signals, definition)
            }
            crate::step::StepRunResult::Failure { error } =>
                self.handle_step_failure(flow_id, cursor, step_def, error),
        }
    }


    fn handle_step_success(
        &mut self,
        flow_id: Uuid,
        cursor: usize,
        step_def: &dyn StepDefinition,
        mut outputs: Vec<Artifact>,
        definition: &FlowDefinition,
    ) -> Result<(), CoreEngineError> {
        let output_hashes = self.hash_and_store_outputs(&mut outputs);
        let fp = self.calculate_step_fingerprint(cursor, step_def, &output_hashes, definition);

        let _finished = self.event_store.append_kind(
            flow_id,
            FlowEventKind::StepFinished {
                step_index: cursor,
                step_id: step_def.id().to_string(),
                outputs: output_hashes.clone(),
                fingerprint: fp.clone(),
            },
        );

        if cursor + 1 == definition.len() {
            self.complete_flow(flow_id, definition);
        }

        Ok(())
    }

    fn handle_step_success_with_signals(
        &mut self,
        flow_id: Uuid,
        cursor: usize,
        step_def: &dyn StepDefinition,
        mut outputs: Vec<Artifact>,
        signals: Vec<crate::step::StepSignal>,
        definition: &FlowDefinition,
    ) -> Result<(), CoreEngineError> {
        let output_hashes = self.hash_and_store_outputs(&mut outputs);
        for s in signals {
            let _ = self.event_store.append_kind(
                flow_id,
                FlowEventKind::StepSignal {
                    step_index: cursor,
                    step_id: step_def.id().to_string(),
                    signal: s.signal,
                    data: s.data,
                },
            );
        }

        let fp = self.calculate_step_fingerprint(cursor, step_def, &output_hashes, definition);

        let _finished = self.event_store.append_kind(
            flow_id,
            FlowEventKind::StepFinished {
                step_index: cursor,
                step_id: step_def.id().to_string(),
                outputs: output_hashes.clone(),
                fingerprint: fp.clone(),
            },
        );

        if cursor + 1 == definition.len() {
            self.complete_flow(flow_id, definition);
        }

        Ok(())
    }

    fn handle_step_failure(
        &mut self,
        flow_id: Uuid,
        cursor: usize,
        step_def: &dyn StepDefinition,
        error: CoreEngineError,
    ) -> Result<(), CoreEngineError> {
        let fp_json = json!({
            "engine_version": crate::constants::ENGINE_VERSION,
            "definition_hash": step_def.definition_hash(),
            "step_index": cursor,
            "params": step_def.base_params(),
        });
        let fp = hash_value(&fp_json);

        let _ = self.event_store.append_kind(
            flow_id,
            FlowEventKind::StepFailed {
                step_index: cursor,
                step_id: step_def.id().to_string(),
                error: error.clone(),
                fingerprint: fp,
            },
        );

        Err(error)
    }

    fn calculate_step_fingerprint(
        &self,
        cursor: usize,
        step_def: &dyn StepDefinition,
        output_hashes: &[String],
        definition: &FlowDefinition,
    ) -> String {
        let fp_json = json!({
            "engine_version": crate::constants::ENGINE_VERSION,
            "definition_hash": definition.definition_hash,
            "step_index": cursor,
            "output_hashes": output_hashes,
            "params": step_def.base_params(),
        });
        hash_value(&fp_json)
    }

    fn complete_flow(&mut self, flow_id: Uuid, definition: &FlowDefinition) {
        let events = self.event_store.list(flow_id);
        let step_fps: Vec<String> = events
            .iter()
            .filter_map(|e| match &e.kind {
                FlowEventKind::StepFinished { fingerprint, .. } => Some(fingerprint.clone()),
                _ => None,
            })
            .collect();

        let flow_fp = hash_value(&json!({
            "engine_version": crate::constants::ENGINE_VERSION,
            "definition_hash": definition.definition_hash,
            "step_fingerprints": step_fps,
        }));

        let _ = self.event_store.append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint: flow_fp });
    }

    // -- Convenience queries ------------------------------------------------------------

    /// Avanza un paso en el flujo por defecto.
    pub fn next(&mut self) -> Result<(), CoreEngineError> {
        let flow_id = self.ensure_default_flow_id();
        // Tomamos temporalmente la definición por defecto para evitar borrows
        // incompatibles entre `self.default_definition` y la llamada a `next_with`.
        let def = match self.default_definition.take() {
            Some(d) => d,
            None => return Err(CoreEngineError::Internal("no default definition configured".into())),
        };

        let res = self.next_with(flow_id, &def);
        // restaurar
        self.default_definition = Some(def);
        res
    }

    /// Lista eventos del flujo por defecto.
    pub fn events(&self) -> Option<Vec<crate::event::FlowEvent>> {
        self.default_flow_id.map(|fid| self.event_store.list(fid))
    }

    /// Lista eventos de un `flow_id` arbitrario desde el `EventStore`.
    pub fn list_events_for(&self, flow_id: Uuid) -> Vec<crate::event::FlowEvent> {
        self.event_store.list(flow_id)
    }

    /// Alias para `list_events_for` para compatibilidad.
    pub fn events_for(&self, flow_id: Uuid) -> Vec<crate::event::FlowEvent> {
        self.list_events_for(flow_id)
    }

    /// Proporciona acceso al event store para casos de uso avanzados.
    pub fn event_store(&self) -> &E {
        &self.event_store
    }

    /// Proporciona acceso mutable al event store para casos de uso avanzados.
    pub fn event_store_mut(&mut self) -> &mut E {
        &mut self.event_store
    }

    /// Variante compacta de eventos (códigos) para el flujo por defecto.
    pub fn event_variants(&self) -> Option<Vec<&'static str>> {
        self.events().map(|events| {
            events
                .iter()
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

    /// Devuelve el fingerprint final del flujo por defecto, si existe.
    pub fn flow_fingerprint(&self) -> Option<String> {
        let evs = self.events()?;
        evs.iter().rev().find_map(|e| match &e.kind {
            FlowEventKind::FlowCompleted { flow_fingerprint } => Some(flow_fingerprint.clone()),
            _ => None,
        })
    }
    /// Crea una rama (branch) a partir de un `parent_flow_id` copiando la
    /// sub-secuencia de eventos hasta (e incluyendo) el último `StepFinished`
    /// del step identificado por `from_step_id`.
    ///
    /// Comportamiento:
    /// - Busca el último `StepFinished` en el flujo padre para `from_step_id`.
    /// - Crea un nuevo `branch_id` (Uuid) y re-emite en el `EventStore` la
    ///   subsecuencia de eventos del padre hasta ese índice (inclusive).
    /// - Añade en el flujo padre un evento `BranchCreated` para marcar la rama.
    ///
    /// Esto permite crear ramas in-memory de forma declarativa sin exponer
    /// detalles de bajo nivel del `EventStore`.
    pub fn branch(
        &mut self,
        parent_flow_id: Uuid,
        definition: &FlowDefinition,
        from_step_id: &str,
        divergence_params_hash: Option<String>,
    ) -> Result<Uuid, CoreEngineError> {
        // Leer eventos del padre
        let events = self.event_store.list(parent_flow_id);

        // Buscar el FlowInitialized del padre (para comparar definition_hash)
        let parent_def_hash_opt = events.iter().find_map(|e| match &e.kind {
            FlowEventKind::FlowInitialized { definition_hash, .. } => Some(definition_hash.clone()),
            _ => None,
        });

        // Buscar último StepFinished para from_step_id
        let mut last_idx: Option<usize> = None;
        for (i, ev) in events.iter().enumerate() {
            if let FlowEventKind::StepFinished { step_id, .. } = &ev.kind {
                if step_id == from_step_id {
                    last_idx = Some(i);
                }
            }
        }

        let idx = match last_idx {
            Some(i) => i,
            None => return Err(CoreEngineError::InvalidBranchSource),
        };

        // Crear branch id
        let branch_id = Uuid::new_v4();

        // Siempre insertamos un FlowInitialized al comienzo de la rama.
        let _ = self.event_store.append_kind(
            branch_id,
            FlowEventKind::FlowInitialized {
                definition_hash: definition.definition_hash.clone(),
                step_count: definition.len(),
            },
        );

        // Si la definición del branch coincide con la del padre (mismo hash)
        // copiamos los eventos relevantes preservando el orden original.
        let same_definition = parent_def_hash_opt.as_deref().map_or(false, |h| h == definition.definition_hash);
        if same_definition {
            // Localizar el índice del FlowInitialized del padre para comenzar
            // a copiar justo después de él (evita duplicarlo).
            let init_idx = events.iter().position(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })).unwrap_or(0);

            // Copiar eventos desde init_idx+1 hasta idx (inclusive), manteniendo orden.
            for (i, ev) in events.iter().enumerate() {
                if i > init_idx && i <= idx {
                    self.event_store.append_kind(branch_id, ev.kind.clone());
                }
            }
        }

        // Notificar en el padre que se creó una rama
        let _ = self.event_store.append_kind(
            parent_flow_id,
            FlowEventKind::BranchCreated {
                branch_id,
                parent_flow_id,
                root_flow_id: parent_flow_id,
                created_from_step_id: from_step_id.to_string(),
                divergence_params_hash,
            },
        );

        Ok(branch_id)
    }

    /// Crea una rama a partir de un índice de step en lugar de su id. La
    /// semántica es análoga a `branch` pero usa directamente el `step_index`.
    ///
    /// Además, durante la copia de eventos valida que los `outputs` referenciados
    /// por `StepFinished` existan en el `artifact_store`; si falta algún
    /// artifact se retorna `CoreEngineError::StorageError`.
    pub fn branch_by_index(
        &mut self,
        parent_flow_id: Uuid,
        definition: &FlowDefinition,
        from_step_index: usize,
        divergence_params_hash: Option<String>,
    ) -> Result<Uuid, CoreEngineError> {
        // Leer eventos del padre
        let events = self.event_store.list(parent_flow_id);

        // Buscar FlowInitialized del padre
        let parent_def_hash_opt = events.iter().find_map(|e| match &e.kind {
            FlowEventKind::FlowInitialized { definition_hash, .. } => Some(definition_hash.clone()),
            _ => None,
        });

        // Buscar el último StepFinished cuyo step_index == from_step_index
        let mut last_idx: Option<usize> = None;
        for (i, ev) in events.iter().enumerate() {
            if let FlowEventKind::StepFinished { step_index, .. } = &ev.kind {
                if *step_index == from_step_index {
                    last_idx = Some(i);
                }
            }
        }

        let idx = match last_idx {
            Some(i) => i,
            None => return Err(CoreEngineError::InvalidBranchSource),
        };

        // Crear branch id
        let branch_id = Uuid::new_v4();

        // Siempre insertamos un FlowInitialized al comienzo de la rama.
        let _ = self.event_store.append_kind(
            branch_id,
            FlowEventKind::FlowInitialized {
                definition_hash: definition.definition_hash.clone(),
                step_count: definition.len(),
            },
        );

        // Si la definición del branch coincide con la del padre copiamos eventos relevantes
        let same_definition = parent_def_hash_opt.as_deref().map_or(false, |h| h == definition.definition_hash);
        if same_definition {
            let init_idx = events.iter().position(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })).unwrap_or(0);

            for (i, ev) in events.iter().enumerate() {
                if i > init_idx && i <= idx {
                    // Si es StepFinished, verificar que los outputs existen en artifact_store
                    if let FlowEventKind::StepFinished { outputs, .. } = &ev.kind {
                        for h in outputs.iter() {
                            if !self.artifact_store.contains_key(h) {
                                return Err(CoreEngineError::StorageError(format!("missing artifact {} when copying branch", h)));
                            }
                        }
                    }
                    self.event_store.append_kind(branch_id, ev.kind.clone());
                }
            }
        }

        // Notificar en el padre que se creó una rama (usamos el id del step si existe)
        let created_from_step_id = definition
            .steps
            .get(from_step_index)
            .map(|s| s.id().to_string())
            .unwrap_or_else(|| format!("idx:{}", from_step_index));

        let _ = self.event_store.append_kind(
            parent_flow_id,
            FlowEventKind::BranchCreated {
                branch_id,
                parent_flow_id,
                root_flow_id: parent_flow_id,
                created_from_step_id,
                divergence_params_hash,
            },
        );

        Ok(branch_id)
    }

    /// Crea un `BranchBuilder` que permite aplicar overrides y ejecutar el
    /// branch de manera ergonómica. El builder mutably presta acceso al
    /// `FlowEngine` por un tiempo corto (lifetime ligado a &mut self).
    pub fn branch_builder<'a>(
        &'a mut self,
        parent_flow_id: Uuid,
        definition: FlowDefinition,
        from_step_id: &str,
        divergence_params_hash: Option<String>,
    ) -> Result<BranchBuilder<'a, E, R>, CoreEngineError> {
        let branch_id = self.branch(parent_flow_id, &definition, from_step_id, divergence_params_hash)?;
        // Devolver un BranchBuilder que posee una copia de la definición para
        // poder ejecutar la rama sin que el caller mantenga la definición.
    Ok(BranchBuilder { engine: self, branch_id, definition })
    }

    /// Versión del builder que acepta un índice de step.
    pub fn branch_builder_by_index<'a>(
        &'a mut self,
        parent_flow_id: Uuid,
        definition: FlowDefinition,
        from_step_index: usize,
        divergence_params_hash: Option<String>,
    ) -> Result<BranchBuilder<'a, E, R>, CoreEngineError> {
        let branch_id = self.branch_by_index(parent_flow_id, &definition, from_step_index, divergence_params_hash)?;
        Ok(BranchBuilder { engine: self, branch_id, definition })
    }
}

/// Builder ergonómico para operar sobre una rama recién creada.
pub struct BranchBuilder<'a, E, R>
where
    E: EventStore,
    R: FlowRepository,
{
    /// Referencia mutable al engine que administra stores y ejecución.
    pub(crate) engine: &'a mut FlowEngine<E, R>,
    /// Identificador de la rama creada.
    pub(crate) branch_id: Uuid,
    /// Definición asociada a esta rama. Se guarda aquí para permitir
    /// ejecutar la rama sin tener que pasar la definición externamente.
    pub(crate) definition: FlowDefinition,
}

impl<'a, E, R> BranchBuilder<'a, E, R>
where
    E: EventStore,
    R: FlowRepository,
{
    /// Devuelve el id de la rama.
    pub fn id(&self) -> Uuid {
        self.branch_id
    }

    /// Inserta un evento arbitrario en la rama y devuelve `self` para encadenar.
    pub fn append_event(&mut self, kind: FlowEventKind) -> &mut Self {
        let _ = self.engine.event_store.append_kind(self.branch_id, kind);
        self
    }

    /// Añade un override de parámetros para un step concreto usando `StepSignal`.
    pub fn override_step_params(&mut self, step_index: usize, step_id: &str, params: serde_json::Value) -> &mut Self {
        let _ = self.engine.event_store.append_kind(
            self.branch_id,
            FlowEventKind::StepSignal {
                step_index,
                step_id: step_id.to_string(),
                signal: "params_override".to_string(),
                data: params,
            },
        );
        self
    }

    /// Almacena un artifact en la cache del engine y devuelve su hash.
    ///
    /// Útil para preparar artifacts que la rama usará como inputs antes de
    /// ejecutar pasos. El artifact se guarda en el `artifact_store` compartido
    /// del `FlowEngine`.
    pub fn store_artifact(&mut self, artifact: Artifact) -> String {
        let hash = crate::hashing::hash_value(&artifact.payload);
        let mut a = artifact.clone();
        a.hash = hash.clone();
        self.engine.store_artifact(a);
        hash
    }

    /// Ejecuta un paso en la rama (devuelve errores del engine si ocurren).
    pub fn step(&mut self, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
        self.engine.next_with(self.branch_id, definition)
    }

    /// Ejecuta la rama hasta completarla y devuelve el `branch_id` cuando
    /// finaliza correctamente.
    pub fn run_to_completion(&mut self) -> Result<Uuid, CoreEngineError> {
        // Usamos la definici f3n almacenada en el builder para ejecutar la rama.
        self.engine.run_flow_to_completion(self.branch_id, &self.definition)
    }

    /// Finaliza el builder y devuelve el `branch_id`.
    pub fn finalize(self) -> Uuid {
        self.branch_id
    }
}

impl Default for FlowEngine<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository> {
    fn default() -> Self {
        Self::new_with_stores(
            crate::event::InMemoryEventStore::default(),
            crate::repo::InMemoryFlowRepository::new(),
        )
    }
}
