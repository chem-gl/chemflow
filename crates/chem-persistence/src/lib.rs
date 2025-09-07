//! chem-persistence
//!
//! Fase 3 – Capa de Abstracción Rust (Diesel) (ver documentación).
//! Objetivo: Proveer implementaciones Postgres de `EventStore` y
//! `FlowRepository` más utilidades de conexión y migraciones. Esta versión
//! inicial expone únicamente los esqueletos (sin lógica de queries) para
//! permitir iteración incremental sin romper contratos del core.
//!
//! Módulos:
//! - `pg`: implementaciones sobre Postgres (append-only event_log y artifacts).
//! - `migrations`: runner embebido de migraciones Diesel.
//! - `config`: carga de configuración desde .env.
//! - `schema`: tablas Diesel declaradas para compilar queries.

pub mod config;
pub mod error;
pub mod migrations;
pub mod pg;
pub mod schema; // generado manualmente para F3

pub use config::init_dotenv;
pub use error::PersistenceError;
pub use pg::{build_dev_pool_from_env, ConnectionProvider, PgEventStore, PgFlowRepository, PgPool, PoolProvider};
