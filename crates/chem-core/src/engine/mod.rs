//! FlowEngine – punto de orquestación. (Esqueleto sin implementación)

use uuid::Uuid;
use std::collections::HashMap;
// use serde_json::json; // reservado para futuras extensiones

use crate::event::{EventStore, FlowEventKind};
use crate::repo::{FlowRepository, FlowDefinition, FlowInstance};
use crate::model::{Artifact, StepFingerprintInput, ExecutionContext};
use crate::hashing::{to_canonical_json, hash_str};
use crate::step::{StepStatus, StepRunResult};
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
        if step_index >= definition.len() { return Err(CoreEngineError::InvalidStepIndex); }
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
        let fp_input = StepFingerprintInput { engine_version: ENGINE_VERSION, step_id: step_def.id(), input_hashes: &input_hashes, params: &params, definition_hash: &definition.definition_hash };
        let fp_json = serde_json::to_value(&fp_input).expect("fingerprint serialize");
        let canonical = to_canonical_json(&fp_json);
        let fingerprint = hash_str(&canonical);
        let ctx = ExecutionContext { inputs: input_artifacts.clone(), params: params.clone() };
        match step_def.run(&ctx) {
            StepRunResult::Success { mut outputs } => {
                let mut output_hashes = Vec::new();
                for o in outputs.iter_mut() {
                    if o.hash.is_empty() { let payload_canonical = to_canonical_json(&o.payload); o.hash = hash_str(&payload_canonical); }
                    self.artifact_store.insert(o.hash.clone(), o.clone());
                    output_hashes.push(o.hash.clone());
                }
                self.event_store.append_kind(flow_id, FlowEventKind::StepFinished { step_index, step_id: step_def.id().to_string(), outputs: output_hashes, fingerprint: fingerprint.clone() });
                if step_index + 1 == definition.len() { self.event_store.append_kind(flow_id, FlowEventKind::FlowCompleted); }
                Ok(())
            }
            StepRunResult::Failure { error } => { self.event_store.append_kind(flow_id, FlowEventKind::StepFailed { step_index, step_id: step_def.id().to_string(), error, fingerprint }); Ok(()) }
        }
    }
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
}
