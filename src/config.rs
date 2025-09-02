//! Configuración central de la aplicación.
//! Carga variables de entorno (.env) y expone una estructura inmutable
//! (`CONFIG`). También provee `create_pool` para obtener un pool de conexiones
//! a PostgreSQL que será usado por el repositorio para persistencia y
//! migraciones.
use once_cell::sync::Lazy;
use std::env;

/// Configuración global de la aplicación (extensible para más secciones:
/// logging, etc.).
pub struct AppConfig {
    /// Configuración específica de base de datos.
    pub database: DatabaseConfig,
}

/// Parámetros de conexión a la base de datos.
pub struct DatabaseConfig {
    /// URL completa de conexión (postgres://...).
    pub url: String,
    /// Número mínimo de conexiones en el pool.
    pub min_connections: u32,
}

/// Instancia global perezosa de configuración, evaluada una sola vez.
pub static CONFIG: Lazy<AppConfig> = Lazy::new(|| {
    let url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let min = env::var("DATABASE_MIN_CONNECTIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    AppConfig { database: DatabaseConfig { url, min_connections: min } }
});

use sqlx::postgres::PgPoolOptions;
use sqlx::Executor;

/// Crea un pool de conexiones PostgreSQL basado en la configuración cargada.
/// Devuelve un `Result` que permite propagar errores de conexión.
pub async fn create_pool() -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    match PgPoolOptions::new().min_connections(CONFIG.database.min_connections).max_connections(10).connect(&CONFIG.database.url).await {
        Ok(pool) => Ok(pool),
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("3D000") => {
            // Database does not exist; attempt to create it.
            eprintln!("Target database not found. Attempting to create it...");
            ensure_database_exists(&CONFIG.database.url).await?;
            // Retry connection after creation
            PgPoolOptions::new().min_connections(CONFIG.database.min_connections).max_connections(10).connect(&CONFIG.database.url).await
        }
        Err(e) => Err(e),
    }
}

/// Ensures the target database exists by connecting to the 'postgres'
/// maintenance DB and issuing CREATE DATABASE.
async fn ensure_database_exists(full_url: &str) -> Result<(), sqlx::Error> {
    // Very lightweight URL parsing: split at last '/' to isolate db name (ignore
    // query params for now) postgres://user:pass@host:port/dbname[?params]
    let (base, db_name) = if let Some(pos) = full_url.rfind('/') {
        let (b, tail) = full_url.split_at(pos);
        let db_part = &tail[1..]; // remove leading '/'
                                  // Remove query if present
        let db_only = db_part.split('?').next().unwrap_or(db_part);
        (b.to_string(), db_only.to_string())
    } else {
        return Ok(());
    };
    if db_name.is_empty() {
        return Ok(());
    }
    // Build admin URL using 'postgres' maintenance DB (fallback to original if
    // already points there)
    let admin_url = if base.ends_with("/postgres") || db_name == "postgres" { full_url.to_string() } else { format!("{}/postgres", base) };
    // Connect to admin DB
    if let Ok(admin_pool) = PgPoolOptions::new().max_connections(1).connect(&admin_url).await {
        // Issue CREATE DATABASE IF NOT EXISTS (Postgres lacks IF NOT EXISTS for CREATE
        // DATABASE pre-15; emulate) We check pg_database first.
        let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM pg_database WHERE datname = $1").bind(&db_name).fetch_one(&admin_pool).await?;
        if exists.0 == 0 {
            // Safe identifier quoting minimal (no special chars assumed). For safety,
            // refuse suspicious names.
            if db_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                let create_stmt = format!("CREATE DATABASE \"{}\"", db_name.replace('"', ""));
                admin_pool.execute(create_stmt.as_str()).await?;
                eprintln!("Database '{}' created automatically", db_name);
            } else {
                eprintln!("Refusing to auto-create database with potentially unsafe name: {}", db_name);
            }
        }
    }
    Ok(())
}
