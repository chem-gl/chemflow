use chem_core::repo::build_flow_definition;
use chem_core::{EventStore, FlowEngine, FlowEventKind, InMemoryEventStore};
use uuid::Uuid;

#[test]
fn integration_smoke_inmemory_store_and_engine() {
    // InMemory event store should allow append and list deterministically
    let mut store = InMemoryEventStore::default();
    let _def = build_flow_definition(&["s1"], vec![]);
    let flow_id = Uuid::new_v4();

    // Append FlowInitialized
    let ev = store.append_kind(flow_id,
                               FlowEventKind::FlowInitialized { definition_hash: "h1".to_string(),
                                                                step_count: 1 });
    assert_eq!(ev.seq, 0);

    // Create engine with the in-memory store and run a zero-step flow (smoke)
    let repo = chem_core::repo::InMemoryFlowRepository::new();
    let engine: FlowEngine<_, _> = FlowEngine::new_with_stores(store, repo);

    // Engine should expose event_store for listing
    let events = engine.event_store().list(flow_id);
    // At least the FlowInitialized should exist
    assert!(events.iter().any(|e| matches!(e.kind, FlowEventKind::FlowInitialized { .. })),
            "FlowInitialized missing");
}
