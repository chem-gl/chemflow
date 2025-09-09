use uuid::Uuid;

#[test]
fn e2e_human_gate_fingerprint_invariance() {
    use chem_adapters::{FamilyHashInjector, PropertiesInjector};
    use chem_core::event::InMemoryEventStore;
    use chem_core::model::ExecutionContext;
    use chem_core::repo::InMemoryFlowRepository;
    use chem_core::step::{StepDefinition, StepRunResult};
    use chem_core::EventStore;
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
            // produce an artifact with a `properties` array so PropertiesInjector has data
            let art = chem_core::model::Artifact { kind: chem_core::model::ArtifactKind::GenericJson,
                                                   hash: String::new(),
                                                   payload: json!({ "properties": [1,2,3], "schema_version": 1 }),
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
            // read injected params to include them in fingerprint
            let _ = ctx.params.get("family_hash");
            let _ = ctx.params.get("properties_summary");
            StepRunResult::Success { outputs: vec![] }
        }
        fn kind(&self) -> chem_core::step::StepKind {
            chem_core::step::StepKind::Transform
        }
    }

    let def = build_flow_definition(&["src", "t"], vec![Box::new(Source), Box::new(T)]);

    // Run #1: no gate
    let flow = Uuid::new_v4();
    let ev = InMemoryEventStore::default();
    let repo = InMemoryFlowRepository::new();
    let mut eng = FlowEngine::new_with_stores(ev, repo);
    eng.injectors.push(Box::new(FamilyHashInjector));
    eng.injectors.push(Box::new(PropertiesInjector));
    eng.next_with(flow, &def).expect("src");
    eng.next_with(flow, &def).expect("t");
    let fp1 = eng.last_step_fingerprint(flow, "t").expect("fp1");

    // Run #2: with simulated human gate: engine will emit UserInteractionRequested
    // if param requires_human_input
    let flow2 = Uuid::new_v4();
    let ev2 = InMemoryEventStore::default();
    let repo2 = InMemoryFlowRepository::new();
    let mut eng2 = FlowEngine::new_with_stores(ev2, repo2);
    eng2.injectors.push(Box::new(FamilyHashInjector));
    eng2.injectors.push(Box::new(PropertiesInjector));

    // run src
    eng2.next_with(flow2, &def).expect("src2");
    // simulate that params indicate a human gate: append event manually and then
    // resume
    eng2.event_store.append_kind(flow2,
                                 chem_core::event::FlowEventKind::UserInteractionRequested { step_index: 1,
                                                                                             step_id: "t".to_string(),
                                                                                             schema: None,
                                                                                             hint: None });
    // now resume with an empty provided (no overrides)
    let provided = json!({});
    let resumed = eng2.resume_user_input(flow2, &def, "t", provided).expect("resume");
    assert!(resumed, "resume should apply");
    // continue
    eng2.next_with(flow2, &def).expect("t2");
    let fp2 = eng2.last_step_fingerprint(flow2, "t").expect("fp2");

    assert_eq!(fp1, fp2,
               "fingerprints must match with/without human gate when no overrides provided");
}
