use chem_core::{EventStore, FlowEventKind};
use chem_persistence::pg::{build_pool, PgEventStore, PoolProvider};
use chem_persistence::config::DbConfig;
use uuid::Uuid;
use serde_json::Value;

#[test]
fn roundtrip_all_variants_enum_json_full() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("skip (no DATABASE_URL)"); return; }
    let cfg = DbConfig::from_env();
    // Fuerza 1x1 para aislar posibles issues en destrucción de múltiples conexiones
    let pool = build_pool(&cfg.url, 1, 1).expect("pool");
    let provider = PoolProvider { pool: pool.clone() };
    let mut store = PgEventStore::new(provider);
    let flow_id = Uuid::new_v4();

    // Construir cada variante con datos sintéticos mínimos.
    let variants: Vec<FlowEventKind> = vec![
        FlowEventKind::FlowInitialized { definition_hash: "defhash".into(), step_count: 3 },
        FlowEventKind::StepStarted { step_index: 0, step_id: "s0".into() },
        FlowEventKind::StepSignal { step_index: 0, step_id: "s0".into(), signal: "ping".into(), data: Value::Null },
        FlowEventKind::StepFinished { step_index: 0, step_id: "s0".into(), outputs: vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()], fingerprint: "fp0".into() },
        FlowEventKind::StepFailed { step_index: 1, step_id: "s1".into(), error: chem_core::errors::CoreEngineError::Internal("boom".into()), fingerprint: "fp1".into() },
        FlowEventKind::FlowCompleted { flow_fingerprint: "flowfp".into() },
    ];

    for k in variants.clone() { store.append_kind(flow_id, k); }
    let stored = store.list(flow_id);
    assert_eq!(stored.len(), variants.len());
    for (expected, got) in variants.iter().zip(stored.iter()) {
        let je = serde_json::to_value(expected).unwrap();
        let jg = serde_json::to_value(&got.kind).unwrap();
        assert_eq!(je, jg, "JSON enum debe ser idéntico tras roundtrip");
    }
    // Drop explícito antes de fin de proceso (diagnóstico segfault en teardown)
    drop(store);
}
