use chem_core::{EventStore, FlowEventKind};
use chem_persistence::config::DbConfig;
use chem_persistence::pg::{build_pool, PgEventStore, PoolProvider};
use uuid::Uuid;

// Ejecuta muchas inserciones StepFinished SIN outputs (no activa inserción de
// artifacts)
#[test]
fn stress_stepfinished_no_outputs() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    let cfg = DbConfig::from_env();
    // Pool 1x1 para reducir variables.
    let pool = build_pool(&cfg.url, 1, 1).expect("pool");
    let provider = PoolProvider { pool };
    let mut store = PgEventStore::new(provider);
    let flow_id = Uuid::new_v4();
    let iters: usize = std::env::var("STRESS_ITERS").ok()
                                                    .and_then(|v| v.parse().ok())
                                                    .unwrap_or(2_000);
    for i in 0..iters {
        store.append_kind(flow_id,
                          FlowEventKind::StepFinished { step_index: i as usize,
                                                        step_id: format!("s{i}"),
                                                        outputs: vec![],
                                                        fingerprint: format!("fp{i}") });
        if i % 500 == 0 {
            eprintln!("no_outputs i={i}");
        }
    }
    let evs = store.list(flow_id);
    assert_eq!(evs.len(), iters, "Debe existir un evento por iteración");
}

// Ejecuta muchas inserciones StepFinished con un output para activar
// inserción/deduplicación artifacts.
#[test]
fn stress_stepfinished_with_artifact() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, 1, 1).expect("pool");
    let provider = PoolProvider { pool };
    let mut store = PgEventStore::new(provider);
    let flow_id = Uuid::new_v4();
    let iters: usize = std::env::var("STRESS_ITERS").ok()
                                                    .and_then(|v| v.parse().ok())
                                                    .unwrap_or(2_000);
    // Usamos hash estable para forzar on_conflict_do_nothing repetido.
    let hash = "f00df00df00df00df00df00df00df00df00df00df00df00df00df00df00df00d".to_string();
    for i in 0..iters {
        store.append_kind(flow_id,
                          FlowEventKind::StepFinished { step_index: i as usize,
                                                        step_id: format!("s{i}"),
                                                        outputs: vec![hash.clone()],
                                                        fingerprint: format!("fp{i}") });
        if i % 500 == 0 {
            eprintln!("with_artifact i={i}");
        }
    }
    let evs = store.list(flow_id);
    assert_eq!(evs.len(), iters, "Debe existir un evento por iteración");
}

// Variante: mezcla lista tras cada append para stress adicional de conexiones y
// deserialización.
#[test]
fn stress_append_and_list_mix() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, 1, 1).expect("pool");
    let provider = PoolProvider { pool };
    let mut store = PgEventStore::new(provider);
    let flow_id = Uuid::new_v4();
    let iters: usize = std::env::var("STRESS_ITERS").ok()
                                                    .and_then(|v| v.parse().ok())
                                                    .unwrap_or(1_000);
    for i in 0..iters {
        store.append_kind(flow_id,
                          FlowEventKind::StepFinished { step_index: i as usize,
                                                        step_id: format!("m{i}"),
                                                        outputs: if i % 2 == 0 {
                                                            vec![]
                                                        } else {
                                                            vec![format!("hash{i:04}")]
                                                        },
                                                        fingerprint: format!("fp{i}") });
        let _current = store.list(flow_id); // fuerza deserialización total
        if i % 200 == 0 {
            eprintln!("mix i={i}");
        }
    }
    let final_events = store.list(flow_id);
    if final_events.len() != iters {
        eprintln!("final_events_len={} expected={}", final_events.len(), iters);
        // Mostrar últimos 10 seq y detectar huecos.
        let mut seqs: Vec<u64> = final_events.iter().map(|e| e.seq).collect();
        seqs.sort();
        let mut gaps = Vec::new();
        for w in seqs.windows(2) {
            if w[1] != w[0] + 1 {
                gaps.push((w[0], w[1]));
            }
        }
        eprintln!("gaps={:?}", gaps);
        eprintln!("last_10={:?}", seqs.iter().rev().take(10).cloned().collect::<Vec<_>>());
        let evens = seqs.iter().filter(|s| *s % 2 == 0).count();
        let odds = seqs.len() - evens;
        eprintln!("parity evens={} odds={}", evens, odds);
    }
    assert_eq!(final_events.len(), iters, "Eventos finales deben coincidir con iteraciones");
}
