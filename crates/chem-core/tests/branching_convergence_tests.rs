use uuid::Uuid;

#[test]
fn branch_fingerprint_convergence_no_param_change() {
    use chem_core::event::InMemoryEventStore;
    use chem_core::model::{Artifact, ArtifactKind, ExecutionContext};
    use chem_core::repo::InMemoryFlowRepository;
    use chem_core::step::{StepDefinition, StepKind, StepRunResult};
    use chem_core::{build_flow_definition, FlowEngine};
    use serde_json::json;

    // Simple deterministic steps
    struct S1;
    impl StepDefinition for S1 {
        fn id(&self) -> &str {
            "s1"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            let art = Artifact { kind: ArtifactKind::GenericJson,
                                 hash: String::new(),
                                 payload: json!({"v": 1, "schema_version": 1}),
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
        fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
            // deterministic transform: propagate input unchanged (identity)
            let input = ctx.input.as_ref().expect("input present").clone();
            StepRunResult::Success { outputs: vec![input] }
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
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            let art = Artifact { kind: ArtifactKind::GenericJson,
                                 hash: String::new(),
                                 payload: json!({"post": "ok", "schema_version": 1}),
                                 metadata: None };
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> StepKind {
            StepKind::Transform
        }
    }

    let ev_store = InMemoryEventStore::default();
    let repo = InMemoryFlowRepository::new();
    let mut engine = FlowEngine::new_with_stores(ev_store, repo);

    let def = build_flow_definition(&["s1", "s2", "s3"], vec![Box::new(S1), Box::new(S2), Box::new(S3)]);
    let parent = Uuid::new_v4();

    // Run s1 and s2 on parent
    engine.next_with(parent, &def).expect("s1 ok");
    engine.next_with(parent, &def).expect("s2 ok");

    // Branch from s2 (no param changes)
    let branch = engine.branch(parent, &def, "s2", None).expect("branch created");

    // Run s3 on parent
    engine.next_with(parent, &def).expect("parent s3 ok");
    let fp_parent = engine.last_step_fingerprint(parent, "s3").expect("fp parent");

    // Run s3 on branch
    engine.next_with(branch, &def).expect("branch s3 ok");
    let fp_branch = engine.last_step_fingerprint(branch, "s3").expect("fp branch");

    assert_eq!(fp_parent, fp_branch,
               "Fingerprints for s3 must match between parent and branch when params didn't change");
}
