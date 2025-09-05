//! chem-persistence
//! 
//! Fase 3 – Capa de Abstracción Rust (Diesel) (ver documentación).
//! Objetivo: Proveer implementaciones Postgres de `EventStore` y `FlowRepository` más
//! utilidades de conexión y migraciones. Esta versión inicial expone únicamente los
//! esqueletos (sin lógica de queries) para permitir iteración incremental sin romper
//! contratos del core.

pub mod error;
pub mod pg;
pub mod config;
pub mod migrations;
pub mod schema; // generado manualmente para F3

pub use pg::{PgEventStore, PgFlowRepository, PgPool, ConnectionProvider};
pub use error::PersistenceError;
pub use config::init_dotenv;
