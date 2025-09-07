use chem_persistence::config::DbConfig;
use chem_persistence::pg::build_pool;
use diesel::RunQueryDsl;

// Test mínimo: sólo crea y descarta un pool (1x1) varias veces.
// Si un segfault aparece aquí, la causa es externa a la lógica de
// eventos/artifacts.
#[test]
fn minimal_pool_create_drop_loop() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    let cfg = DbConfig::from_env();
    let loops: usize = std::env::var("POOL_LOOPS").ok().and_then(|v| v.parse().ok()).unwrap_or(100);
    for i in 0..loops {
        let pool = build_pool(&cfg.url, 1, 1).expect("pool");
        // checkout + simple ping (SELECT 1)
        let mut conn = pool.get().expect("conn");
        let _ = diesel::sql_query("SELECT 1").execute(&mut conn);
        drop(conn);
        drop(pool); // explicit
        if i % 20 == 0 {
            eprintln!("minimal_pool iteration={i}");
        }
    }
}
