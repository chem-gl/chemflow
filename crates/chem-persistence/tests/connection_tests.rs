//! Pruebas básicas de configuración y pool (requiere DATABASE_URL válido en entorno).

use chem_persistence::{config::DbConfig, pg::build_pool};

#[test]
fn create_pool_from_env() {
    if std::env::var("DATABASE_URL").is_err() { eprintln!("DATABASE_URL no definido: omitiendo test"); return; }
    let cfg = DbConfig::from_env();
    let pool = build_pool(&cfg.url, cfg.min_connections, cfg.max_connections).expect("pool");
    let mut conn = pool.get().expect("conn");
    // Sonda trivial de validez (no falla ejecutar un simple query vacio)
    use diesel::connection::SimpleConnection;
    conn.batch_execute("SELECT 1;").expect("select 1");
}
