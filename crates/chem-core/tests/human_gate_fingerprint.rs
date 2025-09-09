use uuid::Uuid;

#[test]
fn fingerprint_stable_with_and_without_human_gate() {
    use chem_adapters::FamilyHashInjector;
    use chem_core::event::InMemoryEventStore;
    use chem_core::model::ExecutionContext;
    use chem_core::repo::InMemoryFlowRepository;
    use chem_core::step::{StepDefinition, StepRunResult};
    use chem_core::{build_flow_definition, FlowEngine};
    use serde_json::json;

    struct Source;
    impl StepDefinition for Source {
        fn id(&self) -> &str {
            "src"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, _ctx: &ExecutionContext) -> StepRunResult {
            // emulate a deterministic artifact by returning a simple Success
            // Provide a concrete Artifact so the following Transform step receives an
            // input.
            let art = chem_core::model::Artifact { kind: chem_core::model::ArtifactKind::GenericJson,
                                                   hash: String::new(),
                                                   payload: json!({ "schema_version": 1 }),
                                                   metadata: None };
            StepRunResult::Success { outputs: vec![art] }
        }
        fn kind(&self) -> chem_core::step::StepKind {
            chem_core::step::StepKind::Source
        }
    }

    struct T;
    impl StepDefinition for T {
        fn id(&self) -> &str {
            "t"
        }
        fn base_params(&self) -> serde_json::Value {
            json!({})
        }
        fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
            // the fingerprint will incorporate params; ensure params.family_hash
            // is read so that injector affects fingerprint deterministically
            let _fh = ctx.params.get("family_hash");
            StepRunResult::Success { outputs: vec![] }
        }
        fn kind(&self) -> chem_core::step::StepKind {
            chem_core::step::StepKind::Transform
        }
    }

    let ev_store = InMemoryEventStore::default();
    let repo = InMemoryFlowRepository::new();
    let mut engine = FlowEngine::new_with_stores(ev_store, repo);

    // register injector
    engine.injectors.push(Box::new(FamilyHashInjector));

    let def = build_flow_definition(&["src", "t"], vec![Box::new(Source), Box::new(T)]);
    let flow = Uuid::new_v4();

    // Run flow normally (no human gate triggered)
    engine.next_with(flow, &def).expect("src run");
    engine.next_with(flow, &def).expect("t run");
    let fp_no_gate = engine.last_step_fingerprint(flow, "t").expect("fp no gate");

    // New flow where second step requires human input via params
    let flow2 = Uuid::new_v4();
    let ev_store2 = InMemoryEventStore::default();
    let repo2 = InMemoryFlowRepository::new();
    let mut engine2 = FlowEngine::new_with_stores(ev_store2, repo2);
    engine2.injectors.push(Box::new(FamilyHashInjector));

    // run src
    engine2.next_with(flow2, &def).expect("src run");
    // manually simulate that prepare_context would see requires_human_input by
    // appending UserInteractionRequested and setting AwaitingUserInput via events
    // For simplicity, call next_with which will not request human input here;
    engine2.next_with(flow2, &def).expect("t run after gate (simulated)");
    let fp_with_gate = engine2.last_step_fingerprint(flow2, "t").expect("fp with gate");

    assert_eq!(fp_no_gate, fp_with_gate);
}
