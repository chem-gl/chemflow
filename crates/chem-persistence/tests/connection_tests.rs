use chem_persistence::config::DbConfig;
mod test_support;
use test_support::with_pool;

#[test]
fn create_pool_from_env() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL no definido: omitiendo test");
        return;
    }
    DbConfig::from_env();
    let pool = with_pool(|p| p.clone());
    if pool.is_none() {
        eprintln!("skip create_pool_from_env (sin pool global)");
        return;
    }
    let pool = pool.unwrap();
    let mut conn = pool.get().expect("conn");
    use diesel::connection::SimpleConnection;
    conn.batch_execute("SELECT 1;\n").expect("select 1");
    if std::env::var("LEAK_POOL").is_ok() {
        std::mem::forget(pool);
        eprintln!("LEAK_POOL activo en connection_tests");
    }
}
