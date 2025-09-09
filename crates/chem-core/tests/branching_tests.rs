use uuid::Uuid;

#[test]
fn branch_emits_event_and_rejects_on_non_finished_source() {
    use chem_core::event::FlowEventKind;
    use chem_core::event::InMemoryEventStore;
    use chem_core::model::{Artifact, ArtifactKind, ExecutionContext};
    use chem_core::repo::InMemoryFlowRepository;
    use chem_core::step::{StepDefinition, StepKind, StepRunResult};
    use chem_core::{build_flow_definition, FlowEngine};
    use serde_json::json;

    // Minimal source step that succeeds
    struct SourceStep;
    impl StepDefinition for SourceStep {
        fn id(&self) -> &str {
            "seed"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &ExecutionContext) -> chem_core::step::StepRunResult {
            let art = Artifact { kind: ArtifactKind::GenericJson,
                                 hash: String::new(),
                                 payload: json!({"v":1, "schema_version":1}),
                                 metadata: None };
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Source
        }
    }

    let mut engine = FlowEngine::new_with_stores(InMemoryEventStore::default(), InMemoryFlowRepository::new());
    let def = build_flow_definition(&["seed"], vec![Box::new(SourceStep)]);
    let flow_id = Uuid::new_v4();

    // Before executing the step, branching should be rejected (source not
    // FinishedOk)
    let err = engine.branch(flow_id, &def, "seed", None).unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("invalid branch") || err_str.contains("InvalidBranchSource") || err_str.contains("step"),
            "expected invalid branch source error, got: {}",
            err_str);

    // Execute the source step so it's FinishedOk
    engine.next_with(flow_id, &def).expect("source should run");

    // Now branch should succeed and emit BranchCreated event
    let branch_id = engine.branch(flow_id, &def, "seed", Some("divhash-demo".to_string()))
                          .expect("branch should be created");
    let events = engine.events_for(flow_id);
    let found = events.into_iter().any(|e| match e.kind {
                                      FlowEventKind::BranchCreated { .. } => true,
                                      _ => false,
                                  });
    assert!(found,
            "BranchCreated event must be present after branch() (branch_id={})",
            branch_id);
}
