//! Wrapper para correr migraciones embebidas.
//! En etapas iniciales solo expone función `run_pending_migrations`.
//!
//! Se espera que exista un directorio `migrations/` en este crate con las
//! migraciones Diesel. Al inicializar el pool se ejecutan una vez.

use crate::error::PersistenceError;
use diesel::connection::SimpleConnection;
use diesel::pg::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

// Directorio esperado: `migrations/` en este crate.
// Para agregar migraciones: `diesel migration generate <name>` (fuera del
// contexto del asistente). Aquí se embeben todas las migraciones.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn run_pending_migrations(conn: &mut PgConnection) -> Result<(), PersistenceError> {
    conn.batch_execute("CREATE EXTENSION IF NOT EXISTS pgcrypto;").ok();
    conn.run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(|e| PersistenceError::Unknown(format!("migration error: {e}")))
}
