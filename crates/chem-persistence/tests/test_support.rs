use chem_persistence::config::DbConfig;
use chem_persistence::pg::{build_pool, PgPool};
use once_cell::sync::Lazy;

pub static TEST_POOL: Lazy<Option<PgPool>> = Lazy::new(|| {
    if std::env::var("DATABASE_URL").is_err() {
        return None;
    }
    let cfg = DbConfig::from_env();
    match build_pool(&cfg.url, 1, 1) {
        // usar 1x1 estable
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("No se pudo construir pool de test: {e}");
            None
        }
    }
});

pub fn with_pool<F, R>(f: F) -> Option<R>
    where F: FnOnce(&PgPool) -> R
{
    TEST_POOL.as_ref().map(|p| f(p))
}
