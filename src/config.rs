use once_cell::sync::Lazy;
use std::env;

pub struct AppConfig {
    pub database: DatabaseConfig,
}

pub struct DatabaseConfig {
    pub url: String,
    pub min_connections: u32,
}

pub static CONFIG: Lazy<AppConfig> = Lazy::new(|| {
    let url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let min = env::var("DATABASE_MIN_CONNECTIONS").ok()
        .and_then(|v| v.parse().ok()).unwrap_or(2);
    AppConfig {
        database: DatabaseConfig { url, min_connections: min },
    }
});

use sqlx::postgres::PgPoolOptions;

pub async fn create_pool() -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    PgPoolOptions::new()
        .min_connections(CONFIG.database.min_connections)
        .max_connections(10)
        .connect(&CONFIG.database.url)
        .await
}
