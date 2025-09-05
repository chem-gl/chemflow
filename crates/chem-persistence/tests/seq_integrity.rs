use chem_core::{EventStore, FlowEventKind, InMemoryEventStore};
use chem_persistence::pg::{build_pool, PgEventStore, PoolProvider};
use chem_persistence::config::DbConfig;
use uuid::Uuid;

// Testea que los seq en Postgres sean contiguos (sin gaps) para un mismo flow_id.
#[test]
fn seq_is_contiguous_for_single_flow() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("skip seq_is_contiguous_for_single_flow (no DATABASE_URL)"); return; }
    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, cfg.min_connections, cfg.max_connections).expect("pool");
    let mut store = PgEventStore::new(PoolProvider { pool });
    let flow_id = Uuid::new_v4();
    // Insertar N eventos
    let n = 6u32;
    for i in 0..n {
        store.append_kind(flow_id, FlowEventKind::StepStarted { step_index: i as usize, step_id: format!("s{i}") });
    }
    let events = store.list(flow_id);
    assert_eq!(events.len(), n as usize, "Debe haber {n} eventos");
    // BIGSERIAL es global a la tabla: sólo exigimos contigüidad relativa al primer seq del flow.
    let base = events.first().map(|e| e.seq).expect("primer evento");
    for (offset, ev) in events.iter().enumerate() {
        let expected = base + offset as u64;
        assert_eq!(ev.seq, expected, "seq debe ser contiguo (esperado {expected} got {} base {base})", ev.seq);
    }
}

// InMemory parity del contrato (también contiguo)
#[test]
fn seq_is_contiguous_inmemory() {
    let mut store = InMemoryEventStore::default();
    let flow_id = Uuid::new_v4();
    for i in 0..5 { store.append_kind(flow_id, FlowEventKind::StepStarted { step_index: i, step_id: format!("s{i}") }); }
    let events = store.list(flow_id);
    for (expected_seq, ev) in (0u64..).zip(events.iter()) { assert_eq!(ev.seq, expected_seq); }
}
