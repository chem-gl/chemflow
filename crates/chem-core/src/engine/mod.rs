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

/// Motor lineal determinista (F2). Mantiene referencias a contratos de almacenamiento.
pub struct FlowEngine<E: EventStore, R: FlowRepository> { pub event_store: E, pub repository: R, pub artifact_store: HashMap<String, Artifact> }

impl<E: EventStore, R: FlowRepository> FlowEngine<E, R> {
    /// Crea un nuevo motor.
    pub fn new(event_store: E, repository: R) -> Self { Self { event_store, repository, artifact_store: HashMap::new() } }

    /// Ejecuta el siguiente step de un flujo.
    pub fn next(&mut self, flow_id: Uuid, definition: &FlowDefinition) -> Result<(), CoreEngineError> {
        let events = self.event_store.list(flow_id);
        if events.is_empty() {
            self.event_store.append_kind(flow_id, FlowEventKind::FlowInitialized { definition_hash: definition.definition_hash.clone(), step_count: definition.len() });
        }
        let events = self.event_store.list(flow_id);
        let instance: FlowInstance = self.repository.load(flow_id, &events, definition);
    if instance.completed { return Err(CoreEngineError::FlowCompleted); }
    let step_index = instance.cursor;
    // Si no hay steps pendientes pero no está marcado completed => flujo detenido por fallo terminal.
    if step_index >= definition.len() { return Err(CoreEngineError::StepAlreadyTerminal); }
        if !matches!(instance.steps[step_index].status, StepStatus::Pending) { return Err(CoreEngineError::StepAlreadyTerminal); }
        let step_def = &definition.steps[step_index];
        let required = step_def.required_input_kinds();
        let mut input_artifacts: Vec<Artifact> = Vec::new();
        for (idx, slot) in instance.steps.iter().enumerate() {
            if idx >= step_index { break; }
            if !matches!(slot.status, StepStatus::FinishedOk) { continue; }
            for h in &slot.outputs {
                if let Some(a) = self.artifact_store.get(h) { if required.iter().any(|k| *k == a.kind) { input_artifacts.push(a.clone()); } }
            }
        }
        for kind in required { if !input_artifacts.iter().any(|a| &a.kind == kind) { return Err(CoreEngineError::MissingInputs); } }
        self.event_store.append_kind(flow_id, FlowEventKind::StepStarted { step_index, step_id: step_def.id().to_string() });
        let params = step_def.base_params();
        let mut input_hashes: Vec<String> = input_artifacts.iter().map(|a| a.hash.clone()).collect(); input_hashes.sort();
        let fingerprint = compute_step_fingerprint(step_def.id(), &input_hashes, &params, &definition.definition_hash);
        let ctx = ExecutionContext { inputs: input_artifacts.clone(), params: params.clone() };
        match step_def.run(&ctx) {
            StepRunResult::Success { mut outputs } => {
                let mut output_hashes = Vec::new();
                for o in outputs.iter_mut() {
                    // (INV_CORE_5) Calcular siempre el hash canonical y validar.
                    let payload_canonical = to_canonical_json(&o.payload);
                    let computed = hash_str(&payload_canonical);
                    if o.hash.is_empty() { o.hash = computed.clone(); }
                    debug_assert_eq!(o.hash, computed, "Artifact hash debe ser hash(canonical_json(payload))");
                    self.artifact_store.insert(o.hash.clone(), o.clone());
                    output_hashes.push(o.hash.clone());
                }
                // No inspeccionamos payload (agnóstico datos). Las señales sólo provienen de SuccessWithSignals.
                self.event_store.append_kind(flow_id, FlowEventKind::StepFinished { step_index, step_id: step_def.id().to_string(), outputs: output_hashes, fingerprint: fingerprint.clone() });
                if step_index + 1 == definition.len() { self.event_store.append_kind(flow_id, FlowEventKind::FlowCompleted); }
                Ok(())
            }
            StepRunResult::SuccessWithSignals { mut outputs, signals } => {
                let mut output_hashes = Vec::new();
                for o in outputs.iter_mut() {
                    let payload_canonical = to_canonical_json(&o.payload);
                    let computed = hash_str(&payload_canonical);
                    if o.hash.is_empty() { o.hash = computed.clone(); }
                    debug_assert_eq!(o.hash, computed, "Artifact hash debe ser hash(canonical_json(payload))");
                    self.artifact_store.insert(o.hash.clone(), o.clone());
                    output_hashes.push(o.hash.clone());
                }
                // Emite señales declaradas por el step (ya que el motor es agnóstico).
                for StepSignal { signal, data } in signals.into_iter() {
                    self.event_store.append_kind(
                        flow_id,
                        FlowEventKind::StepSignal { step_index, step_id: step_def.id().to_string(), signal, data }
                    );
                }
                self.event_store.append_kind(flow_id, FlowEventKind::StepFinished { step_index, step_id: step_def.id().to_string(), outputs: output_hashes, fingerprint: fingerprint.clone() });
                if step_index + 1 == definition.len() { self.event_store.append_kind(flow_id, FlowEventKind::FlowCompleted); }
                Ok(())
            }
            StepRunResult::Failure { error } => { self.event_store.append_kind(flow_id, FlowEventKind::StepFailed { step_index, step_id: step_def.id().to_string(), error, fingerprint }); Ok(()) }
        }
    }
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

    // ----------------------------------------------------------------------------------
    // DEFINICIÓN DE ARTIFACTOS TIPADOS (sin semántica de dominio, sólo datos genéricos)
    // ----------------------------------------------------------------------------------
    /// Artifact producido por el step inicial (Source). Contiene un vector de enteros
    /// y un campo de versionado de esquema para soportar evoluciones futuras.
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
        fn required_input_kinds(&self) -> &[ArtifactKind] { &[] }
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
        fn required_input_kinds(&self) -> &[ArtifactKind] { &[ArtifactKind::GenericJson] }
        fn base_params(&self) -> serde_json::Value { json!({}) }
        fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
            // Uso de tipado fuerte para deserializar el primer artifact.
            use crate::model::TypedArtifact;
            let first = ctx.inputs.first().expect("seed output present");
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
                crate::event::FlowEventKind::FlowCompleted => "FlowCompleted"
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
            crate::event::FlowEventKind::FlowCompleted => "C",
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
            fn required_input_kinds(&self) -> &[ArtifactKind] { &[] }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![SingleOut { v: 42, schema_version: 1 }.into_artifact()] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["single"], vec![Box::new(SingleStep)]);
        engine.next(flow_id, &definition).unwrap();
        let events = engine.event_store.list(flow_id);
    let variants: Vec<_> = events.iter().map(|e| match &e.kind { crate::event::FlowEventKind::FlowInitialized{..}=>"I", crate::event::FlowEventKind::StepStarted{..}=>"S", crate::event::FlowEventKind::StepFinished{..}=>"F", crate::event::FlowEventKind::FlowCompleted=>"C", crate::event::FlowEventKind::StepFailed{..}=>"X", crate::event::FlowEventKind::StepSignal{..}=>"G" }).collect();
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
    fn failure_does_not_advance() {
        struct FailStep; // siempre falla
        impl crate::step::StepDefinition for FailStep {
            fn id(&self) -> &str { "fail" }
            fn required_input_kinds(&self) -> &[ArtifactKind] { &[ArtifactKind::GenericJson] }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Failure { error: CoreEngineError::MissingInputs } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["seed","fail"], vec![Box::new(SeedStep), Box::new(FailStep)]);
        engine.next(flow_id, &definition).unwrap(); // seed ok
        engine.next(flow_id, &definition).unwrap(); // fail step executes
        // intentar de nuevo debe dar StepAlreadyTerminal (slot en Failed)
        let err = engine.next(flow_id, &definition).unwrap_err();
        assert_eq!(err.to_string(), crate::errors::CoreEngineError::StepAlreadyTerminal.to_string());
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
    fn invalid_input_kind() {
        struct NeedsJson; // requiere GenericJson pero flujo tiene step fuente diferente
        impl crate::step::StepDefinition for NeedsJson {
            fn id(&self) -> &str { "needs" }
            fn required_input_kinds(&self) -> &[ArtifactKind] { &[ArtifactKind::GenericJson] }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Transform }
        }
        // Definir sólo el step que necesita input inexistente
        let flow_id = Uuid::new_v4();
        let mut engine = FlowEngine::new(InMemoryEventStore::default(), InMemoryFlowRepository::new());
        let definition = build_flow_definition(&["needs"], vec![Box::new(NeedsJson)]);
        let err = engine.next(flow_id, &definition).unwrap_err();
        assert_eq!(err.to_string(), crate::errors::CoreEngineError::MissingInputs.to_string());
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
            fn required_input_kinds(&self) -> &[ArtifactKind] { &[] }
            fn base_params(&self) -> serde_json::Value { json!({}) }
            fn run(&self, _ctx: &ExecutionContext) -> StepRunResult { StepRunResult::Success { outputs: vec![Acc { value:0, schema_version:1 }.into_artifact()] } }
            fn kind(&self) -> crate::step::StepKind { crate::step::StepKind::Source }
        }

        // Macro para definir steps que suman N al último valor
        macro_rules! inc_step { ($name:ident, $n:expr) => {
            struct $name; impl crate::step::StepDefinition for $name {
                fn id(&self) -> &str { stringify!($name) }
                fn required_input_kinds(&self) -> &[ArtifactKind] { &[ArtifactKind::GenericJson] }
                fn base_params(&self) -> serde_json::Value { json!({"inc": $n}) }
                fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
                    use crate::model::TypedArtifact; let first = ctx.inputs.last().unwrap();
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
}
