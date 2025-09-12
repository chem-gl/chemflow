//! Engine module for FlowEngine implementation
//!
//! Provides the core engine, builder pattern, and flow context for
//! deterministic workflow execution.

pub mod core;
pub mod builder;
pub mod flow_ctx;

pub use core::FlowEngine;
pub use builder::{EngineBuilderInit, EngineBuilder};
pub use flow_ctx::FlowCtx;

pub use crate::event::{EventStore, FlowEvent, FlowEventKind, InMemoryEventStore};
pub use crate::repo::{FlowDefinition, FlowRepository, InMemoryFlowRepository};
pub use crate::step::{StepRunResult, StepStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Artifact, ArtifactKind};
    use crate::step::{StepDefinition, StepKind};
    use serde_json::json;

    // Paso fuente de ejemplo
    #[derive(Debug)]
    struct SourceStep;

    impl StepDefinition for SourceStep {
        fn id(&self) -> &str { "source" }
        fn base_params(&self) -> serde_json::Value { json!({}) }
        fn run(&self, _ctx: &crate::model::ExecutionContext) -> crate::step::StepRunResult {
            crate::step::StepRunResult::Success {
                outputs: vec![Artifact {
                    kind: ArtifactKind::GenericJson,
                    payload: json!({"data": "hello world"}),
                    hash: String::new(),
                    metadata: None,
                }]
            }
        }
        fn kind(&self) -> StepKind { StepKind::Source }
    }

    // Paso transformador de ejemplo
    #[derive(Debug)]
    struct TransformStep;

    impl StepDefinition for TransformStep {
        fn id(&self) -> &str { "transform" }
        fn base_params(&self) -> serde_json::Value { json!({}) }
        fn run(&self, ctx: &crate::model::ExecutionContext) -> crate::step::StepRunResult {
            if let Some(input) = &ctx.input {
                let transformed = json!({
                    "transformed": input.payload["data"],
                    "processed": true
                });
                crate::step::StepRunResult::Success {
                    outputs: vec![Artifact {
                        kind: ArtifactKind::GenericJson,
                        payload: transformed,
                        hash: String::new(),
                        metadata: None,
                    }]
                }
            } else {
                crate::step::StepRunResult::Failure {
                    error: crate::errors::CoreEngineError::MissingInputs
                }
            }
        }
        fn kind(&self) -> StepKind { StepKind::Transform }
    }

    // Paso sumidero de ejemplo
    #[derive(Debug)]
    struct SinkStep;

    impl StepDefinition for SinkStep {
        fn id(&self) -> &str { "sink" }
        fn base_params(&self) -> serde_json::Value { json!({}) }
        fn run(&self, ctx: &crate::model::ExecutionContext) -> crate::step::StepRunResult {
            if let Some(input) = &ctx.input {
                println!("Resultado final: {:?}", input.payload);
                crate::step::StepRunResult::Success { outputs: vec![] }
            } else {
                crate::step::StepRunResult::Failure {
                    error: crate::errors::CoreEngineError::MissingInputs
                }
            }
        }
        fn kind(&self) -> StepKind { StepKind::Sink }
    }

    #[test]
    fn test_flow_engine_builder_pattern() {
        // Crear el engine usando el patrón builder
        let engine = FlowEngine::new()
            .first_step(SourceStep)
            .add_step(TransformStep)
            .add_step(SinkStep)
            .build();

        // Ejecutar el flujo completo
        let flow_id = engine.run().expect("El flujo debería completarse exitosamente");

        // Verificar que se generó un ID de flujo
        assert!(!flow_id.to_string().is_empty());

        // Verificar los eventos generados
        let events = engine.get_events().expect("Deberían existir eventos");
        assert!(!events.is_empty());

        // Verificar las variantes de eventos
        let variants = engine.event_variants().expect("Deberían existir variantes");
        println!("Secuencia de eventos: {:?}", variants);

        // Verificar que el flujo se completó
        assert!(variants.contains(&"C")); // 'C' = FlowCompleted
    }

    #[test]
    fn test_flow_engine_step_by_step() {
        let mut engine = FlowEngine::new()
            .first_step(SourceStep)
            .add_step(TransformStep)
            .add_step(SinkStep)
            .build();

        // Ejecutar paso a paso
        assert!(engine.step().is_ok()); // Primer paso
        assert!(engine.step().is_ok()); // Segundo paso
        assert!(engine.step().is_ok()); // Tercer paso
        assert!(engine.step().is_err()); // El flujo ya se completó

        // Verificar el fingerprint del flujo
        let fingerprint = engine.flow_fingerprint();
        assert!(fingerprint.is_some());
        println!("Fingerprint del flujo: {}", fingerprint.unwrap());
    }

    #[test]
    fn test_flow_context() {
        let mut engine = FlowEngine::new()
            .first_step(SourceStep)
            .add_step(TransformStep)
            .add_step(SinkStep)
            .build();

        let flow_id = engine.ensure_default_flow_id();
        let definition = engine.default_definition.as_ref().unwrap().clone();

        let mut ctx = FlowCtx::new(&mut engine, flow_id, &definition);

        // Ejecutar usando el contexto
        assert!(ctx.step().is_ok());
        assert!(ctx.run_n(2).is_ok()); // Ejecutar los 2 pasos restantes
        assert!(ctx.step().is_err()); // El flujo ya se completó
    }
}
