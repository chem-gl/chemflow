use chem_persistence::pg::{build_pool, PgEventStore, PoolProvider};
use chem_persistence::config::DbConfig;
use chem_core::{EventStore, FlowEventKind};
use uuid::Uuid;
use diesel::prelude::*;

#[test]
fn artifact_dedup_insert_only_once() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("skip (no DATABASE_URL)"); return; }
    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, cfg.min_connections, cfg.max_connections).expect("pool");
    let provider = PoolProvider { pool: pool.clone() };
    let mut store = PgEventStore::new(provider);
    let flow_id = Uuid::new_v4();
    // Dos eventos StepFinished que repiten mismo hash output
    let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    store.append_kind(flow_id, FlowEventKind::StepFinished { step_index:0, step_id: "a".into(), outputs: vec![hash.clone()], fingerprint: "fp1".into() });
    store.append_kind(flow_id, FlowEventKind::StepFinished { step_index:1, step_id: "b".into(), outputs: vec![hash.clone()], fingerprint: "fp2".into() });
    // Contar filas artifact
    let mut conn = pool.get().unwrap();
    #[derive(QueryableByName)]
    struct Count {
        #[sql_type = "diesel::sql_types::BigInt"]
        count: i64,
    }
    let result: Count = diesel::sql_query("SELECT COUNT(*) as count FROM workflow_step_artifacts WHERE artifact_hash = $1")
        .bind::<diesel::sql_types::Text,_>(&hash)
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(result.count, 1, "Artifact duplicado no debe insertarse dos veces");
}
