//! Engine module for FlowEngine implementation
//!
//! Provides the core engine, builder pattern, and flow context for
//! deterministic workflow execution.

pub mod builder;
pub mod core;
pub mod flow_ctx;

pub use builder::{EngineBuilder, EngineBuilderInit};
pub use core::FlowEngine;
pub use flow_ctx::FlowCtx;

pub use crate::event::{EventStore, FlowEvent, FlowEventKind, InMemoryEventStore};
pub use crate::repo::{FlowDefinition, FlowRepository, InMemoryFlowRepository};
pub use crate::step::{StepRunResult, StepStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::StepKind;
    use serde_json::json;

    // Import macros
    use crate::typed_artifact;
    use crate::typed_step;

    // --- Helpers de test: pequeña spec JSON para TypedStep
    // ------------------------- Usamos los macros de ayuda para reducir el
    // boilerplate de tests. Definimos una pequeña spec JSON y tres pasos
    // (Source/Transform/Sink).
    typed_artifact!(JsonSpec { value: serde_json::Value });
    typed_step! {
        source SourceStep {
            id: "source",
            output: JsonSpec,
            params: (),
            run(self, _p) {
                JsonSpec { value: json!({ "data": "hello world" }), schema_version: 1 }
            }
        }
    }

    typed_step! {
        step TransformStep {
            id: "transform",
            kind: StepKind::Transform,
            input: JsonSpec,
            output: JsonSpec,
            params: (),
            run(_self, inp, _p) {
                let transformed = json!({ "transformed": inp.value["data"], "processed": true });
                JsonSpec { value: transformed, schema_version: 1 }
            }
        }
    }

    typed_step! {
        step SinkStep {
            id: "sink",
            kind: StepKind::Sink,
            input: JsonSpec,
            output: JsonSpec,
            params: (),
            run(_self, inp, _p) {
                println!("Resultado final: {:?}", inp.value);
                // retornamos el mismo artifact como output para cumplir la firma
                JsonSpec { value: inp.value.clone(), schema_version: 1 }
            }
        }
    }

    #[test]
    fn test_flow_engine_builder_pattern() {
        // Crear el engine usando el patrón builder
        let mut engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
            .first_step(SourceStep::new())
            .add_step(TransformStep::new())
            .add_step(SinkStep::new())
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
        let mut engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
            .first_step(SourceStep::new())
            .add_step(TransformStep::new())
            .add_step(SinkStep::new())
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
        let mut engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
            .first_step(SourceStep::new())
            .add_step(TransformStep::new())
            .add_step(SinkStep::new())
            .build();

        let flow_id = engine.ensure_default_flow_id();

        let steps: Vec<Box<dyn crate::step::StepDefinition>> = vec![Box::new(SourceStep::new()),
                                                                    Box::new(TransformStep::new()),
                                                                    Box::new(SinkStep::new()),];
        let definition = crate::repo::build_flow_definition_auto(steps);

        let mut ctx = FlowCtx::new(&mut engine, flow_id, &definition);

        // Ejecutar usando el contexto
        assert!(ctx.step().is_ok());
        assert!(ctx.run_n(2).is_ok()); // Ejecutar los 2 pasos restantes
        assert!(ctx.step().is_err()); // El flujo ya se completó
    }
}
