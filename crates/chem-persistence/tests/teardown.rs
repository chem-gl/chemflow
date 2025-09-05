use chem_persistence::pg::build_pool;
use chem_persistence::config::DbConfig;
use diesel::RunQueryDsl;

// Diagnóstico: crear y limpiar explícitamente el pool antes de salir.
// Si este test (en loop) nunca segfault y otros sí, el crash está en rutas adicionales (event/artifact) o múltiple creación.
// Ejecutar: for i in {1..50}; do cargo test -q -p chem-persistence --test teardown -- --test-threads=1 || break; done
#[test]
fn explicit_pool_clear_teardown() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("skip (no DATABASE_URL)"); return; }
    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, 1, 1).expect("pool");
    // Simple ping
    {
        let mut conn = pool.get().expect("conn");
        let _ = diesel::sql_query("SELECT 1").execute(&mut conn);
    }
    // Estado antes de clear
    let state_before = pool.state();
    eprintln!("state_before: connections={} idle={}", state_before.connections, state_before.idle_connections);
    drop(pool);
    eprintln!("teardown test end marker");
    if std::env::var("EXIT_EARLY").is_ok() { std::process::exit(0); }
}
