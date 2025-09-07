//! Carga de configuración de conexión desde variables de entorno.
//! Usa convención `DATABASE_URL` y parámetros opcionales de pool.
//!
//! Variables soportadas:
//! - `DATABASE_URL` (obligatoria)
//! - `DATABASE_MIN_CONNECTIONS` (por defecto: 2)
//! - `DATABASE_MAX_CONNECTIONS` (por defecto: 16)

use dotenvy::dotenv;
use once_cell::sync::Lazy;
use std::env;

// Carga perezosa del archivo .env una sola vez.
static DOTENV_LOADED: Lazy<()> = Lazy::new(|| {
    let _ = dotenv();
});

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub min_connections: u32,
    pub max_connections: u32,
}

impl DbConfig {
    pub fn from_env() -> Self {
        // asegura que .env se haya cargado
        Lazy::force(&DOTENV_LOADED);
        let url = env::var("DATABASE_URL").expect("DATABASE_URL no definido");
        let min_connections = env::var("DATABASE_MIN_CONNECTIONS").ok()
                                                                  .and_then(|v| v.parse().ok())
                                                                  .unwrap_or(2);
        let max_connections = env::var("DATABASE_MAX_CONNECTIONS").ok()
                                                                  .and_then(|v| v.parse().ok())
                                                                  .unwrap_or(16);
        Self { url,
               min_connections,
               max_connections }
    }
}

/// Forzar carga temprana de .env desde aplicaciones externas si se desea.
pub fn init_dotenv() {
    Lazy::force(&DOTENV_LOADED);
}
