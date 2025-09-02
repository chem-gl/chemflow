//! Configuración central de la aplicación.
//! Carga variables de entorno (.env) y expone una estructura inmutable (`CONFIG`).
//! También provee `create_pool` para obtener un pool de conexiones a PostgreSQL
//! que será usado por el repositorio para persistencia y migraciones.
use once_cell::sync::Lazy;
use std::env;

/// Configuración global de la aplicación (extensible para más secciones: logging, etc.).
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
    let min = env::var("DATABASE_MIN_CONNECTIONS").ok()
        .and_then(|v| v.parse().ok()).unwrap_or(2);
    AppConfig {
        database: DatabaseConfig { url, min_connections: min },
    }
});

use sqlx::postgres::PgPoolOptions;

/// Crea un pool de conexiones PostgreSQL basado en la configuración cargada.
/// Devuelve un `Result` que permite propagar errores de conexión.
pub async fn create_pool() -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    PgPoolOptions::new()
        .min_connections(CONFIG.database.min_connections)
        .max_connections(10)
        .connect(&CONFIG.database.url)
        .await
}
