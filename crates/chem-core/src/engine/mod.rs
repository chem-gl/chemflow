//! FlowEngine – punto de orquestación. (Esqueleto sin implementación)

use uuid::Uuid;
use std::collections::HashMap;

use crate::event::{EventStore, FlowEventKind};
use crate::repo::{FlowRepository, FlowDefinition, FlowInstance};
use crate::model::{Artifact, StepFingerprintInput, ExecutionContext};
use crate::hashing::{to_canonical_json, hash_str};
use crate::step::{StepStatus, StepRunResult, StepSignal};
use crate::constants::ENGINE_VERSION;
use crate::errors::CoreEngineError;

/// Estado interno de ejecución de un step antes de serializar a eventos.
struct ExecutionOutcome {
    fingerprint: String,
    output_hashes: Vec<String>,
    signals: Vec<StepSignal>,
    status: ExecutionStatus,
}

enum ExecutionStatus { Success, Failure(CoreEngineError) }

/// Motor lineal determinista (F2). Mantiene referencias a contratos de almacenamiento.
pub struct FlowEngine<E: EventStore, R: FlowRepository> { pub event_store: E, pub repository: R, pub artifact_store: HashMap<String, Artifact> }

impl<E: EventStore, R: FlowRepository> FlowEngine<E, R> {
    /// Crea un nuevo motor.
    pub fn new(event_store: E, repository: R) -> Self { Self { event_store, repository, artifact_store: HashMap::new() } }

    /// Ejecuta el siguiente step de un flujo.
    pub fn next(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
    let instance = self.load_or_init(flow_id, definition);
        let step_index = self.validate_state(&instance, definition)?;
        let (ctx, fingerprint, step_id) = self.prepare_context(&instance, definition, step_index)?;
        // Emit StepStarted antes de ejecutar.
        self.event_store.append_kind(flow_id, FlowEventKind::StepStarted { step_index, step_id: step_id.clone() });
        let step_def = &definition.steps[step_index];
        let outcome = self.execute_step(step_def, ctx, fingerprint.clone());
    self.persist_events(flow_id, step_index, &step_id, &outcome);
        // Si terminó el flow con éxito, emitir FlowCompleted con flow_fingerprint agregado.
        if self.is_flow_completed_successfully(flow_id, definition) {
            let flow_fingerprint = self.compute_flow_fingerprint(flow_id);
            self.event_store.append_kind(flow_id, FlowEventKind::FlowCompleted { flow_fingerprint });
        }
        // refrescar instancia no necesario para F2 (stateless en llamada)
        Ok(())
    }

    fn load_or_init(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> FlowInstance {
        let events = self.event_store.list(flow_id);
        if events.is_empty() {
            self.event_store.append_kind(flow_id, FlowEventKind::FlowInitialized { definition_hash: definition.definition_hash.clone(), step_count: definition.len() });
        }
        let events2 = self.event_store.list(flow_id);
        self.repository.load(flow_id, &events2, definition)
    }

    fn validate_state(&self, instance: &FlowInstance, definition: &FlowDefinition) -> Result<usize, CoreEngineError> {
        if instance.completed { return Err(CoreEngineError::FlowCompleted); }
        if instance.steps.iter().any(|s| matches!(s.status, StepStatus::Failed)) { return Err(CoreEngineError::FlowHasFailed); }
        let idx = instance.cursor;
        if idx >= definition.len() { return Err(CoreEngineError::StepAlreadyTerminal); }
        if !matches!(instance.steps[idx].status, StepStatus::Pending) { return Err(CoreEngineError::StepAlreadyTerminal); }
        Ok(idx)
    }

    fn prepare_context(&self, instance: &FlowInstance, definition: &FlowDefinition, step_index: usize) -> Result<(ExecutionContext, String, String), CoreEngineError> {
        let step_def = &definition.steps[step_index];
        if step_index == 0 && !matches!(step_def.kind(), crate::step::StepKind::Source) { return Err(CoreEngineError::FirstStepMustBeSource); }
        let input_artifact: Option<Artifact> = if step_index == 0 { None } else {
            let prev = &instance.steps[step_index - 1];
            if !matches!(prev.status, StepStatus::FinishedOk) { None } else { prev.outputs.get(0).and_then(|h| self.artifact_store.get(h)).cloned() }
        };
        if step_index > 0 && input_artifact.is_none() { return Err(CoreEngineError::MissingInputs); }
        let params = step_def.base_params();
        let mut input_hashes: Vec<String> = input_artifact.iter().map(|a| a.hash.clone()).collect();
        input_hashes.sort();
        let fingerprint = compute_step_fingerprint(step_def.id(), &input_hashes, &params, &definition.definition_hash);
        let ctx = ExecutionContext { input: input_artifact, params };
        Ok((ctx, fingerprint, step_def.id().to_string()))
    }

    fn execute_step(&mut self, step_def: &Box<dyn crate::step::StepDefinition>, ctx: ExecutionContext, fingerprint: String) -> ExecutionOutcome {
        match step_def.run(&ctx) {
            StepRunResult::Success { mut outputs } => {
                let output_hashes = self.hash_and_store_outputs(&mut outputs);
                ExecutionOutcome { fingerprint, output_hashes, signals: vec![], status: ExecutionStatus::Success }
            }
            StepRunResult::SuccessWithSignals { mut outputs, signals } => {
                let output_hashes = self.hash_and_store_outputs(&mut outputs);
                ExecutionOutcome { fingerprint, output_hashes, signals, status: ExecutionStatus::Success }
            }
            StepRunResult::Failure { error } => ExecutionOutcome { fingerprint, output_hashes: vec![], signals: vec![], status: ExecutionStatus::Failure(error) }
        }
    }

    fn persist_events(&mut self, flow_id: Uuid, step_index: usize, step_id: &str, outcome: &ExecutionOutcome) {
    // Emitir señales sólo en éxito.
        if matches!(outcome.status, ExecutionStatus::Success) {
            for StepSignal { signal, data } in outcome.signals.iter().cloned() {
                self.event_store.append_kind(flow_id, FlowEventKind::StepSignal { step_index, step_id: step_id.to_string(), signal, data });
            }
        }
        match &outcome.status {
            ExecutionStatus::Success => {
                self.event_store.append_kind(flow_id, FlowEventKind::StepFinished { step_index, step_id: step_id.to_string(), outputs: outcome.output_hashes.clone(), fingerprint: outcome.fingerprint.clone() });
            }
            ExecutionStatus::Failure(err) => {
                self.event_store.append_kind(flow_id, FlowEventKind::StepFailed { step_index, step_id: step_id.to_string(), error: err.clone(), fingerprint: outcome.fingerprint.clone() });
            }
        }
        // FlowCompleted emitido por caller luego de verificar todos FinishedOk.
    }

    fn is_flow_completed_successfully(&self, flow_id: Uuid, definition: &FlowDefinition) -> bool {
        let events = self.event_store.list(flow_id);
        // Contar StepFinished y verificar que son == steps.len() y no hay StepFailed; y aún no existe FlowCompleted.
        let mut finished = 0usize; let mut failed = false; let mut completed = false;
        for e in &events {
            match &e.kind {
                FlowEventKind::StepFinished { .. } => finished += 1,
                FlowEventKind::StepFailed { .. } => failed = true,
                FlowEventKind::FlowCompleted { .. } => completed = true,
                _ => {}
            }
        }
        !completed && !failed && finished == definition.len()
    }

    fn compute_flow_fingerprint(&self, flow_id: Uuid) -> String {
        let events = self.event_store.list(flow_id);
        let mut fps: Vec<String> = events.iter().filter_map(|e| match &e.kind { FlowEventKind::StepFinished { fingerprint, .. } => Some(fingerprint.clone()), _ => None }).collect();
        fps.sort();
        let v = serde_json::Value::Array(fps.into_iter().map(serde_json::Value::String).collect());
        let canonical = to_canonical_json(&v);
        hash_str(&canonical)
    }

    #[cfg(test)]
    pub(crate) fn test_compute_flow_fingerprint(&self, flow_id: Uuid) -> String { self.compute_flow_fingerprint(flow_id) }
    fn hash_and_store_outputs(&mut self, outputs: &mut [Artifact]) -> Vec<String> {
        let mut output_hashes = Vec::new();
        for o in outputs.iter_mut() {
            let payload_canonical = to_canonical_json(&o.payload);
            let computed = hash_str(&payload_canonical);
            if o.hash.is_empty() { o.hash = computed.clone(); }
            debug_assert_eq!(o.hash, computed, "Artifact hash debe ser hash(canonical_json(payload))");
            self.artifact_store.insert(o.hash.clone(), o.clone());
            output_hashes.push(o.hash.clone());
        }
        output_hashes
    }
    pub fn get_artifact(&self, hash: &str) -> Option<&Artifact> { self.artifact_store.get(hash) }
}

/// Helper recomendado por especificación (Sección 17) para encapsular cálculo fingerprint.
pub fn compute_step_fingerprint(step_id: &str, input_hashes: &[String], params: &serde_json::Value, definition_hash: &str) -> String {
    let fp_input = StepFingerprintInput { engine_version: ENGINE_VERSION, step_id, input_hashes, params, definition_hash };
    let fp_json = serde_json::to_value(&fp_input).expect("fingerprint serialize");
    let canonical = to_canonical_json(&fp_json);
    hash_str(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};
    use serde_json::json;
    use crate::model::{ArtifactSpec, ArtifactKind};
    use crate::{InMemoryEventStore, InMemoryFlowRepository, build_flow_definition, StepRunResult, step::StepDefinition};
    #[derive(Clone, Serialize, Deserialize)]
    struct SeedOutput { values: Vec<i64>, schema_version: u32 }
    impl ArtifactSpec for SeedOutput { const KIND: ArtifactKind = ArtifactKind::GenericJson; }

    /// Artifact producido por el step de transformación que suma los valores previos.
    #[derive(Clone, Serialize, Deserialize)]
    struct SumOutput { sum: i64, schema_version: u32 }
    impl ArtifactSpec for SumOutput { const KIND: ArtifactKind = ArtifactKind::GenericJson; }

    // -----------------------------------------------------------
    // STEPS (definen interfaz puramente determinista y neutral)
    // -----------------------------------------------------------
    /// Step fuente: no necesita inputs y genera un SeedOutput determinista.
    struct SeedStep;
    impl crate::step::StepDefinition for SeedStep {
        fn id(&self) -> &str { "seed" }
        fn base_params(&self) -> serde_json::Value { json!({"n":2}) } // Param dummy para fingerprint.
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            // Datos deterministas (no tiempo / random) => reproducibilidad garantizada.
            let data = SeedOutput { values: vec![1,2], schema_version: 1 };
            let art = data.into_artifact(); // sin hash todavía; engine lo calcula.
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
    }

    /// Step transformador: consume el output del seed y produce la suma.
    struct SumStep;
    impl crate::step::StepDefinition for SumStep {
        fn id(&self) -> &str { "sum" }
        fn base_params(&self) -> serde_json::Value { json!({}) }
        fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
            // Uso de tipado fuerte para deserializar el primer artifact.
            use crate::model::TypedArtifact;
            let first = ctx.input.as_ref().expect("seed output present");
            let seed = TypedArtifact::<SeedOutput>::decode(first).expect("decode seed");
            let s: i64 = seed.inner.values.iter().sum();
            let out = SumOutput { sum: s, schema_version: 1 };
            StepRunResult::Success { outputs: vec![out.into_artifact()] }
        }
        fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
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
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps: Vec<Box<dyn StepDefinition>> = vec![Box::new(SeedStep), Box::new(SumStep)];
        let ids = ["seed", "sum"]; // Orden define el definition_hash
        let definition = build_flow_definition(&ids, steps);
        engine.next(flow_id, &definition).unwrap(); // step seed
        engine.next(flow_id, &definition).unwrap(); // step sum
        let events_run1 = engine.event_store.list(flow_id);

        // Segundo engine (run #2) – reconstruye sin reutilizar estado previo.
        let mut engine2 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps2: Vec<Box<dyn StepDefinition>> = vec![Box::new(SeedStep), Box::new(SumStep)];
        let definition2 = build_flow_definition(&ids, steps2);
        engine2.next(flow_id, &definition2).unwrap();
        engine2.next(flow_id, &definition2).unwrap();
        let events_run2 = engine2.event_store.list(flow_id);

        // Normalizar eventos a su nombre de variante (ignoramos timestamps y hashes concretos).
    fn simplify(ev: &crate::event::FlowEventKind) -> String {
            match ev {
                crate::event::FlowEventKind::FlowInitialized {..} => "FlowInitialized",
                crate::event::FlowEventKind::StepStarted {..} => "StepStarted",
                crate::event::FlowEventKind::StepFinished {..} => "StepFinished",
                crate::event::FlowEventKind::StepFailed {..} => "StepFailed",
                crate::event::FlowEventKind::StepSignal {..} => "StepSignal",
        crate::event::FlowEventKind::FlowCompleted { .. } => "FlowCompleted"
            }.to_string()
        }
        let seq1: Vec<String> = events_run1.iter().map(|e| simplify(&e.kind)).collect();
        let seq2: Vec<String> = events_run2.iter().map(|e| simplify(&e.kind)).collect();
        assert_eq!(seq1, seq2, "Event sequences must match deterministically");

        // Obtener fingerprint del step final ("sum") en ambos runs y compararlos.
        let fp1 = events_run1.iter().find_map(|e| if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind { if step_id=="sum" {Some(fingerprint.clone())} else {None} } else { None });
        let fp2 = events_run2.iter().find_map(|e| if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind { if step_id=="sum" {Some(fingerprint.clone())} else {None} } else { None });
        assert_eq!(fp1, fp2, "Fingerprints must be stable");

        // Nota: Podríamos también validar que los hashes de artifacts producidos coinciden.
        // Si fingerprint es igual y la lógica es pura, esa igualdad se mantiene.
    }

    // ----------------------------------------------------------------------------------
    // TEST 1b (G1 específico): Tres ejecuciones idénticas comparando secuencias de eventos.
    // ----------------------------------------------------------------------------------
    #[test]
    fn determinism_three_runs_event_sequence() {
        let flow_id = Uuid::new_v4();
        let ids = ["seed","sum"];
        // Run 1
        let mut e1 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def1 = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
        e1.next(flow_id, &def1).unwrap(); e1.next(flow_id, &def1).unwrap();
        // Run 2
        let mut e2 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def2 = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
        e2.next(flow_id, &def2).unwrap(); e2.next(flow_id, &def2).unwrap();
        // Run 3
        let mut e3 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def3 = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
        e3.next(flow_id, &def3).unwrap(); e3.next(flow_id, &def3).unwrap();
        let seq = |evs: &[crate::event::FlowEvent]| evs.iter().map(|e| match &e.kind { 
            crate::event::FlowEventKind::FlowInitialized { .. } => "I",
            crate::event::FlowEventKind::StepStarted { .. } => "S",
            crate::event::FlowEventKind::StepFinished { .. } => "F",
            crate::event::FlowEventKind::StepFailed { .. } => "X",
            crate::event::FlowEventKind::StepSignal { .. } => "G", // generic signal
            crate::event::FlowEventKind::FlowCompleted { .. } => "C",
        }).collect::<Vec<_>>();
        let s1 = seq(&e1.event_store.list(flow_id));
        let s2 = seq(&e2.event_store.list(flow_id));
        let s3 = seq(&e3.event_store.list(flow_id));
        assert_eq!(s1, s2, "Run1 vs Run2");
        assert_eq!(s2, s3, "Run2 vs Run3");
    }

    // ----------------------------------------------------------------------------------
    // TEST 1c (G2): Todos los fingerprints de todos los steps coinciden entre 3 runs.
    // ----------------------------------------------------------------------------------
    #[test]
    fn all_step_fingerprints_equal_across_three_runs() {
        let flow_id = Uuid::new_v4();
        let ids = ["seed","sum"];
        let run = |flow_id| {
            let mut e = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
            let def = build_flow_definition(&ids, vec![Box::new(SeedStep), Box::new(SumStep)]);
            e.next(flow_id, &def).unwrap(); e.next(flow_id, &def).unwrap();
            e.event_store.list(flow_id)
        };
        let ev1 = run(flow_id);
        let ev2 = run(flow_id);
        let ev3 = run(flow_id);
    let fps = |evs: &[crate::event::FlowEvent]| {
            evs.iter().filter_map(|e| if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind { Some((step_id.clone(), fingerprint.clone())) } else { None })
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
        #[derive(Clone, Serialize, Deserialize)] struct SingleOut { v: i32, schema_version: u32 }
        impl ArtifactSpec for SingleOut { const KIND: ArtifactKind = ArtifactKind::GenericJson; }
        struct SingleStep;
        impl crate::step::StepDefinition for SingleStep {
            fn id(&self) -> &str { "single" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![SingleOut { v: 42, schema_version: 1 }.into_artifact()] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["single"], vec![Box::new(SingleStep)]);
        engine.next(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
    let variants: Vec<_> = events.iter().map(|e| match &e.kind { crate::event::FlowEventKind::FlowInitialized{..}=>"I", crate::event::FlowEventKind::StepStarted{..}=>"S", crate::event::FlowEventKind::StepFinished{..}=>"F", crate::event::FlowEventKind::FlowCompleted{..}=>"C", crate::event::FlowEventKind::StepFailed{..}=>"X", crate::event::FlowEventKind::StepSignal{..}=>"G" }).collect();
        assert_eq!(variants, vec!["I","S","F","C"], "Secuencia esperada para un sólo step");
    }

    // ----------------------------------------------------------------------------------
    // TEST 3: Dos steps lineales (happy path) – verifica hashes de output no vacíos.
    // ----------------------------------------------------------------------------------
    #[test]
    fn run_linear_two_steps() {
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps: Vec<Box<dyn StepDefinition>> = vec![Box::new(SeedStep), Box::new(SumStep)];
        let definition = build_flow_definition(&["seed","sum"], steps);
        engine.next(flow_id, &definition).unwrap();
        engine.next(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
        let finished = events.iter().filter(|e| matches!(e.kind, crate::event::FlowEventKind::StepFinished{..})).count();
        assert_eq!(finished, 2, "Deben terminar dos steps");
    }

    // ----------------------------------------------------------------------------------
    // TEST 4: Fingerprint estabilidad explícita (comparación directa string).
    // ----------------------------------------------------------------------------------
    #[test]
    fn fingerprint_stability() {
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["seed","sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        engine.next(flow_id, &definition).unwrap();
        engine.next(flow_id, &definition).unwrap();
        let fp1 = engine.event_store.list(flow_id).iter().find_map(|e| if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind { if step_id=="sum" {Some(fingerprint.clone())} else {None} } else { None });
        // run 2
        let mut engine2 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition2 = build_flow_definition(&["seed","sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        engine2.next(flow_id, &definition2).unwrap(); engine2.next(flow_id, &definition2).unwrap();
        let fp2 = engine2.event_store.list(flow_id).iter().find_map(|e| if let crate::event::FlowEventKind::StepFinished { step_id, fingerprint, .. } = &e.kind { if step_id=="sum" {Some(fingerprint.clone())} else {None} } else { None });
        assert_eq!(fp1, fp2, "Fingerprint debe coincidir");
    }

    // ----------------------------------------------------------------------------------
    // TEST 5: Fallo no avanza cursor (step 2 falla y no se re-ejecuta).
    // ----------------------------------------------------------------------------------
    #[test]
    fn failure_stops_following_steps() {
        struct FailStep; // siempre falla
        impl crate::step::StepDefinition for FailStep {
            fn id(&self) -> &str { "fail" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Failure { error: CoreEngineError::MissingInputs } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["seed","fail"], vec![Box::new(SeedStep), Box::new(FailStep)]);
        engine.next(flow_id, &definition).unwrap(); // seed ok
        engine.next(flow_id, &definition).unwrap(); // fail step executes
    // intentar de nuevo debe dar FlowHasFailed (stop-on-failure)
    let err = engine.next(flow_id, &definition).unwrap_err();
    assert_eq!(err.to_string(), crate::errors::CoreEngineError::FlowHasFailed.to_string());
    }

    // ----------------------------------------------------------------------------------
    // TEST 9: No se puede ejecutar después de FlowCompleted (error FlowCompleted).
    // ----------------------------------------------------------------------------------
    #[test]
    fn cannot_run_after_completion() {
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["single"], vec![Box::new(SeedStep)]); // SeedStep actúa como single
        engine.next(flow_id, &def).unwrap(); // ejecuta y completa
        let err = engine.next(flow_id, &def).unwrap_err();
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
            fn id(&self) -> &str { "transform_first" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["transform_first"], vec![Box::new(TransformFirst)]);
    let err = engine.next(flow_id, &definition).unwrap_err();
    assert_eq!(err.to_string(), CoreEngineError::FirstStepMustBeSource.to_string());
    }

    // ----------------------------------------------------------------------------------
    // TEST 10: Cadena de steps sumaN que acumulan y generan StepSignal EVEN_SUM si valor es par.
    // sum1 (+1)=>1 impar NO señal, sum2 (+2)=>3 impar, sum3 (+3)=>6 par señal, sum4 (+4)=>10 par señal.
    // Verifica orden de acumulación y captura de señales.
    // ----------------------------------------------------------------------------------
    #[test]
    fn chained_increment_steps_with_even_signals() {
        #[derive(Clone, Serialize, Deserialize)] struct Acc { value: i64, schema_version: u32 }
        impl ArtifactSpec for Acc { const KIND: ArtifactKind = ArtifactKind::GenericJson; }

        // Step base generador inicial valor 0 (sin señales)
        struct Start; impl crate::step::StepDefinition for Start {
            fn id(&self) -> &str { "sum_start" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![Acc { value:0, schema_version:1 }.into_artifact()] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
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
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let steps: Vec<Box<dyn StepDefinition>> = vec![
            Box::new(Start), Box::new(SumaStep1), Box::new(SumaStep2), Box::new(SumaStep3), Box::new(SumaStep4)
        ];
        let ids = ["sum_start","sumastep1","sumastep2","sumastep3","sumastep4"];
        let definition = build_flow_definition(&ids, steps);
        // Ejecutar todos los steps
        for _ in 0..ids.len() { engine.next(flow_id, &definition).unwrap(); }
        let events = engine.event_store.list(flow_id);
        // Extraer señales EVEN_SUM
        let mut signals: Vec<i64> = events.iter().filter_map(|e| if let FlowEventKind::StepSignal { signal, data, .. } = &e.kind { if signal=="EVEN_SUM" { data.get("value").and_then(|v| v.as_i64()) } else { None } } else { None }).collect();
        signals.sort();
        assert_eq!(signals, vec![6,10], "Deben existir señales para valores pares 6 y 10");
        // Verificar valor final 10 en último StepFinished
        let final_value = events.iter().rev().find_map(|e| if let FlowEventKind::StepFinished { step_id, .. } = &e.kind { if step_id=="sumastep4" {
            // Recuperar artifact correspondiente
            // buscamos hash del output en ese evento y lo resolvemos del artifact_store
            if let FlowEventKind::StepFinished { outputs, .. } = &e.kind { outputs.first().cloned() } else { None }
        } else { None } } else { None });
        // Necesitamos leer artifact_store interno -> acceso directo (no ideal en prod, aceptable test).
        if let Some(h) = final_value { let art = engine.artifact_store.get(&h).unwrap(); assert_eq!(art.payload.get("value").unwrap().as_i64().unwrap(), 10); }
    }

    // ----------------------------------------------------------------------------------
    // TEST 11: Emite una señal personalizada PRINT_HELLO y el test imprime "hola" al verla.
    // Demuestra cómo un consumidor (el test) puede reaccionar a StepSignal sin lógica en el engine.
    // ----------------------------------------------------------------------------------
    #[test]
    fn signal_triggers_side_effect_print_hello() {
        #[derive(Clone, Serialize, Deserialize)] struct Dummy { v: i32, schema_version: u32 }
        impl ArtifactSpec for Dummy { const KIND: ArtifactKind = ArtifactKind::GenericJson; }
        struct HelloSignalStep;
        impl crate::step::StepDefinition for HelloSignalStep {
            fn id(&self) -> &str { "hello_signal" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                let art = Dummy { v: 1, schema_version: 1 }.into_artifact();
                StepRunResult::SuccessWithSignals { outputs: vec![art], signals: vec![StepSignal { signal: "PRINT_HELLO".to_string(), data: json!({}) }] }
            }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["hello_signal"], vec![Box::new(HelloSignalStep)]);
        engine.next(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
        let mut found = false;
        for e in events {
            if let FlowEventKind::StepSignal { signal, .. } = e.kind {
                if signal == "PRINT_HELLO" { println!("hola"); found = true; }
            }
        }
        assert!(found, "Debe haberse emitido la señal PRINT_HELLO");
    }

    // ----------------------------------------------------------------------------------
    // TEST 7: canonical_json ordering produce mismo hash para objetos con claves invertidas.
    // ----------------------------------------------------------------------------------
    #[test]
    fn canonical_json_ordering() {
        use crate::hashing::{to_canonical_json, hash_value};
        let a = json!({"b":2,"a":1});
        let b = json!({"a":1,"b":2});
        assert_eq!(to_canonical_json(&a), to_canonical_json(&b));
        assert_eq!(hash_value(&a), hash_value(&b));
    }

    // ----------------------------------------------------------------------------------
    // TEST 8 (G5): 20 iteraciones de canonical_json sobre construcciones con orden variable.
    // Verifica estabilidad absoluta del hash.
    // ----------------------------------------------------------------------------------
    #[test]
    fn canonical_json_repetition_20() {
        use crate::hashing::{to_canonical_json, hash_value};
        let base = json!({"k1":1, "k2": {"z": true, "a": false}, "array": [3,2,1]});
        let expected_canonical = to_canonical_json(&base);
        let expected_hash = hash_value(&base);
        // Generamos permutaciones deterministas (rotaciones) de las claves recreando Value manualmente.
        let orderings = vec![vec!["k1","k2","array"], vec!["k2","array","k1"], vec!["array","k1","k2"], vec!["array","k2","k1"], vec!["k2","k1","array"], vec!["k1","array","k2" ]];
        for i in 0..20 {
            let ord = &orderings[i % orderings.len()];
            let mut map = serde_json::Map::new();
            for k in ord {
                match *k { "k1" => { map.insert((*k).to_string(), json!(1)); }, "k2" => { map.insert((*k).to_string(), json!({"z": true, "a": false})); }, "array" => { map.insert((*k).to_string(), json!([3,2,1])); }, _ => unreachable!() }
            }
            let v = serde_json::Value::Object(map);
            assert_eq!(to_canonical_json(&v), expected_canonical, "Canonical JSON mismatch iteration {i}");
            assert_eq!(hash_value(&v), expected_hash, "Hash mismatch iteration {i}");
        }
    }

    // ----------------------------------------------------------------------------------
    // TEST 12: Flujo de 2 steps: StepSeven produce número 7; StepReemite lo recibe (typed) y
    // re-emite nuevamente el 7 y además un segundo artifact con mensaje "hola como estas".
    // Verifica paso de artifacts y contenido múltiple.
    // ----------------------------------------------------------------------------------
    #[test]
    fn two_step_number_and_message_flow() {
        #[derive(Clone, Serialize, Deserialize)] struct Numero { value: i64, schema_version: u32 }
        #[derive(Clone, Serialize, Deserialize)] struct Mensaje { msg: String, schema_version: u32 }
        use crate::model::TypedArtifact;
        impl ArtifactSpec for Numero { const KIND: ArtifactKind = ArtifactKind::GenericJson; }
        impl ArtifactSpec for Mensaje { const KIND: ArtifactKind = ArtifactKind::GenericJson; }

        struct StepSeven; impl StepDefinition for StepSeven {
            fn id(&self) -> &str { "step_seven" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
                StepRunResult::Success { outputs: vec![Numero { value:7, schema_version:1 }.into_artifact()] }
            }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }

        struct StepReemite; impl StepDefinition for StepReemite {
            fn id(&self) -> &str { "step_reemite" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                let num_art = ctx.input.as_ref().unwrap();
                let n = TypedArtifact::<Numero>::decode(num_art).unwrap();
                assert_eq!(n.inner.value, 7, "Debe recibir 7");
                let a1 = Numero { value: n.inner.value, schema_version:1 }.into_artifact();
                let a2 = Mensaje { msg: "hola como estas".to_string(), schema_version:1 }.into_artifact();
                StepRunResult::Success { outputs: vec![a1, a2] }
            }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }

        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["step_seven","step_reemite"], vec![Box::new(StepSeven), Box::new(StepReemite)]);
        engine.next(flow_id, &definition).unwrap(); // step 0
        engine.next(flow_id, &definition).unwrap(); // step 1
        let events = engine.event_store.list(flow_id);
        // localizar StepFinished del segundo step
    let finished = events.iter().find(|e| match &e.kind { FlowEventKind::StepFinished { step_id, .. } if step_id == "step_reemite" => true, _ => false }).unwrap();
        let output_hashes = if let FlowEventKind::StepFinished { outputs, .. } = &finished.kind { outputs.clone() } else { vec![] };
        assert_eq!(output_hashes.len(), 2, "Debe producir dos artifacts (numero y mensaje)");
        // decodificar ambos
        let mut have_number7 = false; let mut have_message = false;
        for h in output_hashes {
            let art = engine.artifact_store.get(&h).unwrap();
            if art.payload.get("value").and_then(|v| v.as_i64()) == Some(7) { have_number7 = true; }
            if art.payload.get("msg").and_then(|v| v.as_str()) == Some("hola como estas") { have_message = true; }
        }
        assert!(have_number7 && have_message, "Deben existir el 7 y el mensaje");
    }

    // ----------------------------------------------------------------------------------
    // TEST 13: Flujo con señal: StepSeven produce 7; StepDetect emite StepSignal HAY_UN_7 y
    // en vez de reenviar 7 produce un 9. StepConsume recibe 9 y valida.
    // Verifica paso de artifacts transformados y emisión de señal.
    // ----------------------------------------------------------------------------------
    #[test]
    fn signal_and_transform_number_flow() {
        #[derive(Clone, Serialize, Deserialize)] struct Numero { value: i64, schema_version: u32 }
        use crate::model::TypedArtifact;
        impl ArtifactSpec for Numero { const KIND: ArtifactKind = ArtifactKind::GenericJson; }

        struct StepSeven; impl StepDefinition for StepSeven {
            fn id(&self) -> &str { "step_seven2" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![Numero { value:7, schema_version:1 }.into_artifact()] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }

        struct StepDetect; impl StepDefinition for StepDetect {
            fn id(&self) -> &str { "step_detect" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                let first = ctx.input.as_ref().unwrap();
                let num = TypedArtifact::<Numero>::decode(first).unwrap();
                if num.inner.value == 7 {
                    let out = Numero { value: 9, schema_version:1 }.into_artifact();
                    StepRunResult::SuccessWithSignals { outputs: vec![out], signals: vec![StepSignal { signal: "HAY_UN_7".to_string(), data: json!({"original":7}) }] }
                } else {
                    StepRunResult::Success { outputs: vec![Numero { value: num.inner.value, schema_version:1 }.into_artifact()] }
                }
            }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }

        struct StepConsume; impl StepDefinition for StepConsume {
            fn id(&self) -> &str { "step_consume" }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                // Puede haber múltiples artifacts previos (7 original + 9 transformado). Tomamos el último (más reciente).
                let latest = ctx.input.as_ref().unwrap();
                let num = TypedArtifact::<Numero>::decode(latest).unwrap();
                assert_eq!(num.inner.value, 9, "Debe recibir 9 transformado");
                StepRunResult::Success { outputs: vec![latest.clone()] } // re-emite artifact transformado
            }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["step_seven2","step_detect","step_consume"], vec![Box::new(StepSeven), Box::new(StepDetect), Box::new(StepConsume)]);
        engine.next(flow_id, &definition).unwrap(); // produce 7
        engine.next(flow_id, &definition).unwrap(); // detect -> señal + 9
        engine.next(flow_id, &definition).unwrap(); // consume 9
        let events = engine.event_store.list(flow_id);
        assert!(events.iter().any(|e| matches!(e.kind, FlowEventKind::StepSignal { ref signal, .. } if signal=="HAY_UN_7")), "Debe emitirse señal HAY_UN_7");
        let last_finished = events.iter().rev().find(|e| match &e.kind { FlowEventKind::StepFinished { step_id, .. } if step_id=="step_consume" => true, _ => false }).expect("missing consume finish");
        if let FlowEventKind::StepFinished { outputs, .. } = &last_finished.kind { assert_eq!(outputs.len(),1); }
    }

    // ----------------------------------------------------------------------------------
    // TEST 14: definition_hash depende sólo de ids (orden) – snapshot simple.
    // ----------------------------------------------------------------------------------
    #[test]
    fn definition_hash_only_ids() {
    struct A; impl StepDefinition for A { fn id(&self)->&str{"a"} fn base_params(&self)->serde_json::Value{json!({"x":1})} fn run(&self,_:&ExecutionContext)->StepRunResult{StepRunResult::Success{outputs:vec![]}} fn kind(&self)->crate::step::StepKind{crate::step::StepKind::Source}}
    struct B; impl StepDefinition for B { fn id(&self)->&str{"b"} fn base_params(&self)->serde_json::Value{json!({"y":2})} fn run(&self,_:&ExecutionContext)->StepRunResult{StepRunResult::Success{outputs:vec![]}} fn kind(&self)->crate::step::StepKind{crate::step::StepKind::Transform}}
        let def1 = build_flow_definition(&["a","b"], vec![Box::new(A), Box::new(B)]);
        // Cambiamos parámetros internos pero mismo orden de ids -> mismo hash.
    struct A2; impl StepDefinition for A2 { fn id(&self)->&str{"a"} fn base_params(&self)->serde_json::Value{json!({"x":999})} fn run(&self,_:&ExecutionContext)->StepRunResult{StepRunResult::Success{outputs:vec![]}} fn kind(&self)->crate::step::StepKind{crate::step::StepKind::Source}}
    struct B2; impl StepDefinition for B2 { fn id(&self)->&str{"b"} fn base_params(&self)->serde_json::Value{json!({"y":0})} fn run(&self,_:&ExecutionContext)->StepRunResult{StepRunResult::Success{outputs:vec![]}} fn kind(&self)->crate::step::StepKind{crate::step::StepKind::Transform}}
        let def2 = build_flow_definition(&["a","b"], vec![Box::new(A2), Box::new(B2)]);
        assert_eq!(def1.definition_hash, def2.definition_hash, "definition_hash debe depender solo de ids");
        // Cambiar orden ids cambia hash.
        let def_swapped = build_flow_definition(&["b","a"], vec![Box::new(B2), Box::new(A2)]);
        assert_ne!(def1.definition_hash, def_swapped.definition_hash, "Cambiar orden ids debe cambiar hash");
    }

    // ----------------------------------------------------------------------------------
    // TEST 15: Flow fingerprint agregado determinista entre runs idénticos.
    // ----------------------------------------------------------------------------------
    #[test]
    fn aggregated_flow_fingerprint_deterministic() {
        let flow_id = Uuid::new_v4();
        let mut e1 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def = build_flow_definition(&["seed","sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        e1.next(flow_id, &def).unwrap(); e1.next(flow_id, &def).unwrap();
        let fp1 = e1.test_compute_flow_fingerprint(flow_id);
        let mut e2 = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let def2 = build_flow_definition(&["seed","sum"], vec![Box::new(SeedStep), Box::new(SumStep)]);
        e2.next(flow_id, &def2).unwrap(); e2.next(flow_id, &def2).unwrap();
        let fp2 = e2.test_compute_flow_fingerprint(flow_id);
        assert_eq!(fp1, fp2, "Flow fingerprint agregado debe ser estable");
    }
}
