//! chem-core: Motor lineal determinista (F2)
pub mod constants;
pub mod engine;
pub mod errors;
pub mod event;
pub mod hashing;
pub mod model;
pub mod repo;
pub mod step;
pub mod injection;


pub use engine::{FlowCtx, FlowEngine};
pub use event::{EventStore, FlowEvent, FlowEventKind, InMemoryEventStore};
pub use model::{Artifact, ArtifactKind};
pub use repo::{build_flow_definition, FlowDefinition, FlowRepository, InMemoryFlowRepository};
pub use step::{Pipe, SameAs, StepDefinition, StepKind, StepRunResult, StepRunResultTyped, StepStatus, TypedStep};

// Re-export macros for convenience (they're already exported via #[macro_export])
// pub use step::macros::{typed_artifact, typed_step};

pub use injection::{CompositeInjector, ParamInjector};

#[cfg(test)]
mod tests {
	use super::*;
	use crate::event::FlowEventKind;
	use crate::repo::build_flow_definition_auto;
	use crate::step::StepDefinition;
	use crate::model::Artifact;
	use serde_json::json;

	// Use macros to declare a small typed artifact and three typed steps
	typed_artifact!(JsonSpec { value: serde_json::Value });

	typed_step! {
		source SourceStep {
			id: "src",
			output: JsonSpec,
			params: (),
			run(self, _p) {
				JsonSpec { value: serde_json::json!("src"), schema_version: 1 }
			}
		}
	}

	typed_step! {
		step AStep {
			id: "a",
			kind: StepKind::Transform,
			input: JsonSpec,
			output: JsonSpec,
			params: (),
			run(self, _inp, _p) {
				JsonSpec { value: serde_json::json!("a"), schema_version: 1 }
			}
		}
	}

	typed_step! {
		step BStep {
			id: "b",
			kind: StepKind::Sink,
			input: JsonSpec,
			output: JsonSpec,
			params: (),
			run(self, _inp, _p) {
				JsonSpec { value: serde_json::json!("b"), schema_version: 1 }
			}
		}
	}

	// Variante modificada de AStep que produce una salida distinta. Usamos
	// el mismo `id: "a"` para simular que cambiamos la implementación de
	// un step y luego creamos una rama que usa esta implementación.
	typed_step! {
		step AStepModified {
			id: "a",
			kind: StepKind::Transform,
			input: JsonSpec,
			output: JsonSpec,
			params: Option<String>,
			run(self, _inp, _p) {
				JsonSpec { value: serde_json::json!("a_modified"), schema_version: 1 }
			}
		}
	}

	#[test]
	fn branch_in_memory_copy_creates_branch_and_marks_parent() {
		// Crear el engine de forma ergonómica: src -> a -> b
		let mut parent_engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
			.first_step(SourceStep::new())
			.add_step(AStep::new())
			.add_step(BStep::new())
			.build();

		// Ejecutar el flujo padre para generar eventos en la store in-memory
		let parent_flow_id = parent_engine.run().expect("parent flow should complete");

		// Crear la rama a partir de nuevos pasos (owned) usando la API ergonómica
		let branch_steps: Vec<Box<dyn StepDefinition>> = vec![
			Box::new(SourceStep::new()),
			Box::new(AStep::new()),
			Box::new(BStep::new()),
		];

		let branch_id = parent_engine
			.create_branch_from_steps(parent_flow_id, branch_steps, "a")
			.expect("should create and run branch");

		// Validar: la store de la rama contiene FlowInitialized y StepFinished para "a"
		let branch_events = parent_engine.list_events_for(branch_id);
		assert!(branch_events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })), "branch should have FlowInitialized");
		assert!(branch_events.iter().any(|e| matches!(&e.kind, FlowEventKind::StepFinished { step_id, .. } if step_id == "a")), "branch should contain StepFinished for 'a'");

		// Validar: eventos del padre incluyen BranchCreated apuntando a la rama
		let parent_events_after = parent_engine.list_events_for(parent_flow_id);
		assert!(parent_events_after.iter().any(|e| matches!(e.kind, FlowEventKind::BranchCreated { branch_id: bid, .. } if bid == branch_id)), "parent should include BranchCreated for the new branch");
	}

	#[test]
	fn branch_with_modified_step_produces_different_output() {
		// Crear el engine padre: src -> a -> b
		let mut engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
			.first_step(SourceStep::new())
			.add_step(AStep::new())
			.add_step(BStep::new())
			.build();

		// Ejecutar el flujo padre
		let parent_flow_id = engine.run().expect("parent should complete");

		// Extraer fingerprint del paso 'a' en el padre
		let parent_events = engine.list_events_for(parent_flow_id);
		let parent_a_fp = parent_events
			.iter()
			.find_map(|e| match &e.kind {
				FlowEventKind::StepFinished { step_id, fingerprint, .. } if step_id == "a" => Some(fingerprint.clone()),
				_ => None,
			})
			.expect("parent should have StepFinished for 'a'");

		// Crear una rama usando BranchBuilder directamente para evitar la lógica de copia automática
		let branch_steps: Vec<Box<dyn StepDefinition>> = vec![
			Box::new(SourceStep::new()),
			Box::new(AStepModified::new()),
			Box::new(BStep::new()),
		];
		let def = build_flow_definition_auto(branch_steps);

		let mut builder = engine.branch_builder(parent_flow_id, def, "a", Some("modified".to_string())).expect("branch builder");

		// Ejecutar la rama hasta completarla
		let branch_id = builder.run_to_completion().expect("branch should run");

		// Verificar que la rama tiene FlowInitialized y un StepFinished para 'a'
		let branch_events = engine.list_events_for(branch_id);
		assert!(branch_events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })), "branch should have FlowInitialized");
		let branch_a_fp = branch_events
			.iter()
			.find_map(|e| match &e.kind {
				FlowEventKind::StepFinished { step_id, fingerprint, .. } if step_id == "a" => Some(fingerprint.clone()),
				_ => None,
			})
			.expect("branch should have StepFinished for 'a'");

		// Los fingerprints deben diferir porque la implementación del paso cambió
		assert_ne!(parent_a_fp, branch_a_fp, "branch 'a' fingerprint should differ from parent after modification");

		// Verificar que el padre contiene BranchCreated apuntando a la rama
		let parent_events_after = engine.list_events_for(parent_flow_id);
		assert!(parent_events_after.iter().any(|e| matches!(e.kind, FlowEventKind::BranchCreated { branch_id: bid, .. } if bid == branch_id)), "parent should include BranchCreated for the new branch");
	}

	#[test]
	fn branch_change_metadata_at_step2() {
		// Construir engine padre: src -> a -> b
		let mut engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
			.first_step(SourceStep::new())
			.add_step(AStep::new())
			.add_step(BStep::new())
			.build();

		// Ejecutar el flujo padre para generar eventos y artifacts
		let parent_flow_id = engine.run().expect("parent should complete");

		// Preparar una definición identica para la rama (podría obtenerse de otra fuente)
		let steps: Vec<Box<dyn StepDefinition>> = vec![
			Box::new(SourceStep::new()),
			Box::new(AStep::new()),
			Box::new(BStep::new()),
		];
		let def = build_flow_definition_auto(steps);

		// Get the base params before moving def
		let base_params = def.steps[1].base_params();
		let definition_hash = def.definition_hash.clone();

		// Crear el BranchBuilder desde el paso 'a' (paso index 1)
		let mut builder = engine.branch_builder(parent_flow_id, def, "a", None).expect("branch builder");

		// Crear y almacenar un artifact con metadata distinta que queremos que use la rama
		let art = Artifact::new_unhashed(ArtifactKind::GenericJson, json!({"value": "a", "meta": "branch-mod", "schema_version": 1}), Some(json!({"note": "modified in branch"})));
		let new_hash = builder.store_artifact(art);

		// Calcular el fingerprint que correspondería al StepFinished para 'a'
		let fp_json = json!({
			"engine_version": crate::constants::ENGINE_VERSION,
			"definition_hash": definition_hash,
			"step_index": 1,
			"output_hashes": [new_hash.clone()],
			"params": base_params,
		});
		let fp = crate::hashing::hash_value(&fp_json);

		// Insertar en la rama un StepFinished para 'a' con el nuevo hash
		builder.append_event(FlowEventKind::StepFinished {
			step_index: 1,
			step_id: "a".to_string(),
			outputs: vec![new_hash.clone()],
			fingerprint: fp.clone(),
		});

		// Ejecutar la rama hasta completarla
		let branch_id = builder.run_to_completion().expect("branch should run");

		// Verificar que la rama contiene el StepFinished con el nuevo output hash
		let branch_events = engine.list_events_for(branch_id);
		assert!(branch_events.iter().any(|e| matches!(&e.kind, FlowEventKind::StepFinished { step_id, outputs, fingerprint, .. } if step_id == "a" && outputs.contains(&new_hash) && *fingerprint == fp)), "branch must include StepFinished for 'a' with modified artifact");

		// Verificar que el padre incluye BranchCreated apuntando a la rama
		let parent_events_after = engine.list_events_for(parent_flow_id);
		assert!(parent_events_after.iter().any(|e| matches!(e.kind, FlowEventKind::BranchCreated { branch_id: bid, .. } if bid == branch_id)), "parent should include BranchCreated for the new branch");
	}

	#[test]
	fn branch_create_by_index_example() {
		let mut engine = FlowEngine::<crate::event::InMemoryEventStore, crate::repo::InMemoryFlowRepository>::new()
			.first_step(SourceStep::new())
			.add_step(AStep::new())
			.add_step(BStep::new())
			.build();

		let parent_flow_id = engine.run().expect("parent should complete");

		let steps: Vec<Box<dyn StepDefinition>> = vec![
			Box::new(SourceStep::new()),
			Box::new(AStep::new()),
			Box::new(BStep::new()),
		];

		// Crear rama pasando el índice 1 (step 'a') de forma declarativa
		let branch_id = engine.create_branch_from_steps_at_index(parent_flow_id, steps, 1).expect("create branch by index");

		let branch_events = engine.list_events_for(branch_id);
		assert!(branch_events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })), "branch should have FlowInitialized");

		let parent_events_after = engine.list_events_for(parent_flow_id);
		assert!(parent_events_after.iter().any(|e| matches!(e.kind, FlowEventKind::BranchCreated { branch_id: bid, .. } if bid == branch_id)), "parent should include BranchCreated for the new branch");
	}
}
