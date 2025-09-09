use uuid::Uuid;

#[test]
fn branch_partial_clone_copies_events_up_to_step() {
    use chem_core::event::{FlowEventKind, InMemoryEventStore};
    use chem_core::model::{Artifact, ArtifactKind};
    use chem_core::repo::InMemoryFlowRepository;
    use chem_core::step::{StepDefinition, StepKind, StepRunResult, StepStatus};
    use chem_core::{build_flow_definition, FlowEngine};
    use serde_json::json;

    struct S1;
    impl StepDefinition for S1 {
        fn id(&self) -> &str {
            "s1"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> StepRunResult {
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

    struct S2;
    impl StepDefinition for S2 {
        fn id(&self) -> &str {
            "s2"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> StepRunResult {
            StepRunResult::Success { outputs: vec![] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Transform
        }
    }

    struct S3;
    impl StepDefinition for S3 {
        fn id(&self) -> &str {
            "s3"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &chem_core::model::ExecutionContext) -> StepRunResult {
            StepRunResult::Success { outputs: vec![] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Transform
        }
    }

    let ev_store = InMemoryEventStore::default();
    let repo = InMemoryFlowRepository::new();
    let mut engine = FlowEngine::new_with_stores(ev_store, repo);

    let def = build_flow_definition(&["s1", "s2", "s3"], vec![Box::new(S1), Box::new(S2), Box::new(S3)]);
    let flow_id = Uuid::new_v4();

    // run s1 and s2 so they are FinishedOk
    engine.next_with(flow_id, &def).expect("s1 should run");
    engine.next_with(flow_id, &def).expect("s2 should run");

    // Branch from s2 (should copy events up to s2 StepFinished)
    let branch_id = engine.branch(flow_id, &def, "s2", None).expect("branch created");

    // Events for branch should include initial events up to s2
    let branch_events = engine.events_for(branch_id);
    // Expect at least FlowInitialized and StepFinished for s1 and s2
    let mut has_s1_finished = false;
    let mut has_s2_finished = false;
    for e in branch_events.iter() {
        match &e.kind {
            FlowEventKind::StepFinished { step_id, .. } => {
                if step_id == "s1" {
                    has_s1_finished = true;
                }
                if step_id == "s2" {
                    has_s2_finished = true;
                }
            }
            _ => {}
        }
    }

    assert!(has_s1_finished, "branch must contain s1 finished");
    assert!(has_s2_finished, "branch must contain s2 finished");

    // The parent flow must have a BranchCreated event
    let parent_events = engine.events_for(flow_id);
    let found_branch_evt = parent_events.into_iter()
                                        .any(|e| matches!(e.kind, FlowEventKind::BranchCreated { .. }));
    assert!(found_branch_evt, "parent must have BranchCreated event");
}
