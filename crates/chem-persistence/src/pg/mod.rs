//! Implementaciones Postgres (Diesel) de los traits del core.
//!
//! Objetivo general del módulo:
//! - Proveer una capa de persistencia durable (Postgres) con paridad 1:1
//!   respecto al backend en memoria.
//! - Mantener determinismo del motor: el replay de eventos debe reconstruir el
//!   mismo estado y fingerprints.
//! - Aislar completamente el mapeo dominio ↔ filas de DB del `chem-core`.
//!
//! Estado F5 (Persistencia Postgres mínima, paridad 1:1):
//! - EventStore append-only con orden total por `seq` (BIGSERIAL), sin updates
//!   ni deletes.
//! - Lectura por `flow_id` ordenada por `seq`, equivalente al backend
//!   in-memory.
//! - Inserción opcional de artifacts de step dentro de la MISMA transacción del
//!   evento `StepFinished` (atomicidad cuando el feature de artifacts está
//!   activo; desactivable con feature `no-artifact-insert`).
//! - Manejo básico de errores transitorios: reintento con backoff en `append` y
//!   `list`.
//! - `PgFlowRepository`: delega el replay a la implementación InMemory para
//!   asegurar paridad exacta.

use chem_core::repo::FlowInstance;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use serde_json::Value;
use uuid::Uuid;

use chem_core::errors::{classify_error, ErrorClass};
use chem_core::{EventStore, FlowDefinition, FlowEvent, FlowEventKind, FlowRepository, InMemoryFlowRepository};
use log::{debug, error, warn};

use crate::error::PersistenceError;
use crate::migrations::run_pending_migrations;
use crate::schema::{event_log, step_execution_errors, workflow_step_artifacts};

/// Alias de tipo para el pool r2d2 de conexiones Postgres.
///
/// Notas operativas:
/// - El pool se construye con `min_idle` (mínimo de conexiones inactivas) y
///   `max_size` (límite superior total).
/// - Al construirlo, se corre automáticamente el set de migraciones pendientes
///   (una sola vez).
pub type PgPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Trait interno para obtener una conexión (para testear fácilmente).
/// Proveedor abstracto de conexiones.
///
/// Este trait permite:
/// - Inyectar un pool real (producción/tests de integración).
/// - Simular/factorear en tests unitarios sin acoplar a r2d2.
///
/// Contrato:
/// - Debe devolver una conexión válida o
///   `PersistenceError::TransientIo`/equivalente en caso de error.
pub trait ConnectionProvider: Send + Sync + 'static {
    /// Obtiene una conexión lista para ejecutar consultas Diesel.
    fn connection(&self) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, PersistenceError>;
}

/// Implementación de provider a partir de un pool r2d2.
/// Implementación concreta de `ConnectionProvider` respaldada por un `PgPool`.
pub struct PoolProvider {
    pub pool: PgPool,
}
impl ConnectionProvider for PoolProvider {
    fn connection(&self) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, PersistenceError> {
        self.pool
            .get()
            .map_err(|e| PersistenceError::TransientIo(format!("pool error: {e}")))
    }
}

/// Row mapeada de la tabla `step_execution_errors` (shape mínima anticipada).
/// Fila mapeada de la tabla `step_execution_errors` para lecturas.
///
/// Campos:
/// - `id`: identificador único.
/// - `flow_id`: correlación del flujo.
/// - `step_id`: identificador del step.
/// - `attempt_number`: número de intento.
/// - `error_class`: clasificación del error.
/// - `details`: JSONB con detalles.
/// - `ts`: timestamp.
#[derive(Queryable, Debug)]
pub struct ErrorRow {
    pub id: i64,
    pub flow_id: uuid::Uuid,
    pub step_id: String,
    pub attempt_number: i32,
    pub error_class: String,
    pub details: Option<Value>,
    pub ts: DateTime<Utc>,
}

/// Estructura para inserción (NewEventRow) - `RETURNING` seq, ts.
/// Estructura para inserción en `event_log`.
///
/// Se inserta siempre dentro de una transacción Diesel
/// (`build_transaction().read_write()`), devolviendo `seq` y `ts` vía
/// `RETURNING`.
#[derive(Insertable, Debug)]
#[diesel(table_name = event_log)]
pub struct NewEventRow<'a> {
    pub flow_id: &'a uuid::Uuid,
    pub event_type: &'a str,
    pub payload: &'a Value,
}

/// Fila para insertar artifact (deduplicación por hash via ON CONFLICT DO
/// NOTHING lógica manual).
/// Fila para insertar en `workflow_step_artifacts`.
///
/// - `artifact_hash` funge como PK para deduplicación (length=64 verificado por
///   CHECK).
/// - `produced_in_seq` referencia el `seq` del evento `StepFinished` que lo
///   produjo (FK con `ON DELETE RESTRICT`).
#[derive(Insertable, Debug)]
#[diesel(table_name = workflow_step_artifacts)]
pub struct NewArtifactRow<'a> {
    pub artifact_hash: &'a str,
    pub kind: &'a str,
    pub payload: &'a Value,
    pub metadata: Option<&'a Value>,
    pub produced_in_seq: i64,
}

/// Row mapeada de la tabla `event_log` (shape mínima anticipada).
/// Fila mapeada de la tabla `event_log` para lecturas.
///
/// Campos:
/// - `seq`: identificador monotónico (BIGSERIAL) del evento, global a la tabla.
/// - `flow_id`: correlación del flujo al que pertenece el evento.
/// - `ts`: timestamp asignado por la base de datos (DEFAULT now()).
/// - `event_type`: pista/constraint (minúsculas) del tipo de evento.
/// - `payload`: JSONB con la representación completa del enum `FlowEventKind`.
#[derive(Queryable, Debug)]
pub struct EventRow {
    pub seq: i64,
    pub flow_id: uuid::Uuid,
    pub ts: DateTime<Utc>,
    pub event_type: String,
    pub payload: Value,
}

/// Fila para insertar error de ejecución de step.
/// Fila para insertar en `step_execution_errors`.
///
/// - `flow_id`: correlación del flujo.
/// - `step_id`: identificador del step que falló.
/// - `attempt_number`: número de intento (retry_count).
/// - `error_class`: clasificación del error ('validation', 'runtime', etc.).
/// - `details`: JSONB con detalles del error.
#[derive(Insertable, Debug)]
#[diesel(table_name = step_execution_errors)]
pub struct NewErrorRow<'a> {
    pub flow_id: &'a uuid::Uuid,
    pub step_id: &'a str,
    pub attempt_number: i32,
    pub error_class: &'a str,
    pub details: Option<&'a Value>,
}

/// Determina si un error es transitorio (recomendado reintentar con backoff).
///
/// Cubre:
/// - Conflictos de serialización (deadlocks y nivel de aislamiento).
/// - Errores de IO transitorios de pool/conexión.
/// - Mensajes comunes de desconexión/timeout detectados por texto
///   (best-effort).
fn is_retryable(e: &PersistenceError) -> bool {
    match e {
        PersistenceError::SerializationConflict => true,
        PersistenceError::TransientIo(_) => true,
        // Algunos mensajes de error (dependen de driver/pg) pueden llegar como Unknown
        // con texto. Hacemos best-effort string match sin acoplar a SQLSTATE.
        PersistenceError::Unknown(msg) => {
            let m = msg.to_lowercase();
            m.contains("deadlock detected")
            || m.contains("could not serialize access due to concurrent update")
            || m.contains("terminating connection due to administrator command")
            || m.contains("connection closed")
            || m.contains("connection refused")
            || m.contains("timeout")
        }
        _ => false,
    }
}

/// Retry simple con backoff exponencial muy pequeño (hasta 3 intentos).
///
/// Política:
/// - Intentos: 3.
/// - Backoff: 15ms, 30ms, 45ms.
/// - Logs: se emite `warn!` por intento.
///
/// Garantías:
/// - No altera semántica de negocio; sólo repite la unidad de trabajo provista
///   por `f`.
fn with_retry<F, T>(mut f: F) -> Result<T, PersistenceError>
    where F: FnMut() -> Result<T, PersistenceError>
{
    let mut attempts = 0;
    loop {
        match f() {
            Err(e) if is_retryable(&e) && attempts < 3 => {
                let delay_ms = 15 * ((attempts + 1) as u64);
                warn!("retryable error (attempt {}): {:?} -> sleeping {}ms",
                      attempts + 1,
                      e,
                      delay_ms);
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                attempts += 1;
            }
            r => return r,
        }
    }
}

// SERIALIZACIÓN: guardamos el enum completo como JSON (payload), y además
// persistimos `event_type` (minúsculas) para cumplir constraint y facilitar
// ciertas consultas.
fn serialize_full_enum(kind: &FlowEventKind) -> Value {
    serde_json::to_value(kind).expect("serialize FlowEventKind")
}

/// Mapea la variante del enum a un string en minúsculas, estable en el tiempo.
fn event_type_for(kind: &FlowEventKind) -> &'static str {
    match kind {
        FlowEventKind::FlowInitialized { .. } => "flowinitialized",
        FlowEventKind::StepStarted { .. } => "stepstarted",
        FlowEventKind::StepFinished { .. } => "stepfinished",
        FlowEventKind::StepFailed { .. } => "stepfailed",
        FlowEventKind::StepSignal { .. } => "stepsignal",
        FlowEventKind::PropertyPreferenceAssigned { .. } => "propertypreferenceassigned",
        FlowEventKind::RetryScheduled { .. } => "retryscheduled",
        FlowEventKind::BranchCreated { .. } => "branchcreated",
        FlowEventKind::UserInteractionRequested { .. } => "userinteractionrequested",
        FlowEventKind::UserInteractionProvided { .. } => "userinteractionprovided",
        FlowEventKind::FlowCompleted { .. } => "flowcompleted",
    }
}

/// Deserializa una `EventRow` a `FlowEvent`, utilizando el JSON completo del
/// enum almacenado en `payload`. Si por alguna razón el JSON no es válido,
/// devuelve `None`.
fn deserialize_full_enum(row: EventRow) -> Option<FlowEvent> {
    // Aceptamos cualquiera de los tipos válidos; payload siempre es JSON del enum
    // completo.
    let kind: FlowEventKind = serde_json::from_value(row.payload).ok()?;
    Some(FlowEvent { seq: row.seq as u64,
                     flow_id: row.flow_id,
                     kind,
                     ts: row.ts })
}

/// Implementación Postgres de `EventStore` (append-only).
///
/// Responsabilidades:
/// - `append_kind`: insertar un evento y, opcionalmente, artifacts producidos
///   en el mismo commit.
/// - `list`: devolver todos los eventos de un flow ordenados por `seq` (replay
///   determinista).
pub struct PgEventStore<P: ConnectionProvider> {
    pub provider: P,
}
impl<P: ConnectionProvider> PgEventStore<P> {
    /// Crea un `PgEventStore` a partir de un `ConnectionProvider` (generalmente
    /// `PoolProvider`).
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

impl<P: ConnectionProvider> EventStore for PgEventStore<P> {
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent {
        debug!("append_kind:start flow_id={flow_id} kind={}", kind_variant_name(&kind));
        let event_type = event_type_for(&kind);
        let payload = serialize_full_enum(&kind);
        // Transacción atómica: inserción de evento y (si aplica) artifacts.
        // - Si falla cualquiera de las inserciones, se revierte todo.
        // - Se usa retry/backoff para errores transitorios.
        let inserted: (i64, DateTime<Utc>) = with_retry(|| {
            let mut conn = self.provider.connection()?;
            conn.build_transaction()
                .read_write()
                .run(|tx_conn| {
                    // Paso 1: insertar el evento
                    let (seq, ts): (i64, DateTime<Utc>) = diesel::insert_into(event_log::table)
                        .values(NewEventRow { flow_id: &flow_id, event_type, payload: &payload })
                        .returning((event_log::seq, event_log::ts))
                        .get_result(tx_conn)?;

                    // Paso 2: insertar artifacts asociados (si feature activo)
                    #[cfg(not(feature = "no-artifact-insert"))]
                    {
                        if let FlowEventKind::StepFinished { outputs, .. } = &kind {
                            if !outputs.is_empty() {
                                for h in outputs {
                                    if h.len() != 64 {
                                        debug!("skip artifact hash len!=64 hash={h}");
                                        continue;
                                    }
                                    let null = Value::Null; // snapshot de payload/metadata diferido en F5
                                    let row = NewArtifactRow { artifact_hash: h,
                                                               kind: "unknown",
                                                               payload: &null,
                                                               metadata: None,
                                                               produced_in_seq: seq };
                                    // Dedupe por PK (artifact_hash)
                                    diesel::insert_into(workflow_step_artifacts::table)
                                        .values(&row)
                                        .on_conflict_do_nothing()
                                        .execute(tx_conn)?;
                                }
                            }
                        }
                    }

                    // Paso 3: insertar error si es StepFailed (F8)
                    // Persiste detalles del error para auditoría granular y reconstrucción de timeline.
                    // attempt_number simplificado a 1; futuro: contar StepStarted previos.
                    if let FlowEventKind::StepFailed { step_id, error, .. } = &kind {
                        let error_class = match classify_error(error) {
                            ErrorClass::Runtime => "runtime",
                            ErrorClass::Validation => "validation",
                            ErrorClass::Transient => "transient",
                            ErrorClass::Permanent => "permanent",
                        };
                        let details = serde_json::to_value(error).ok();
                        let attempt_number = 1; // Simplificación: primer intento; en producción, calcular basado en eventos previos
                        let error_row = NewErrorRow {
                            flow_id: &flow_id,
                            step_id,
                            attempt_number,
                            error_class,
                            details: details.as_ref(),
                        };
                        diesel::insert_into(step_execution_errors::table)
                            .values(&error_row)
                            .execute(tx_conn)?;
                    }

                    // Paso 4: insertar metadata de rama si es BranchCreated (F9)
                    if let FlowEventKind::BranchCreated { branch_id, parent_flow_id, root_flow_id, created_from_step_id, divergence_params_hash } = &kind {
                        // Tabla workflow_branches (branch_id PK)
                        // Insert minimal row; metadata puede incluir nombre y JSON adicional más tarde.
                        diesel::sql_query("INSERT INTO workflow_branches (branch_id, root_flow_id, parent_flow_id, created_from_step_id, divergence_params_hash, created_at) VALUES ($1, $2, $3, $4, $5, now()) ON CONFLICT DO NOTHING")
                            .bind::<diesel::sql_types::Uuid, _>(*branch_id)
                            .bind::<diesel::sql_types::Uuid, _>(*root_flow_id)
                            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(Some(*parent_flow_id))
                            .bind::<diesel::sql_types::Text, _>(created_from_step_id.clone())
                            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(divergence_params_hash.clone())
                            .execute(tx_conn)?;
                    }

                    Ok::<(i64, DateTime<Utc>), diesel::result::Error>((seq, ts))
                })
                .map_err(PersistenceError::from)
        })
        .expect("insert event (with artifacts)");

        let ev = FlowEvent { seq: inserted.0 as u64,
                             flow_id,
                             kind,
                             ts: inserted.1 };
        debug!("append_kind:done flow_id={flow_id} seq={} kind={}",
               ev.seq,
               kind_variant_name(&ev.kind));
        ev
    }
    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent> {
        debug!("list:start flow_id={flow_id}");
        // Lectura robusta con retry ante fallos transitorios.
        let rows: Vec<EventRow> = with_retry(|| {
                                      let mut conn = self.provider.connection()?;
                                      let query = event_log::table.filter(event_log::flow_id.eq(flow_id))
                                                                  .order(event_log::seq.asc());
                                      query.load(&mut conn).map_err(PersistenceError::from)
                                  }).unwrap_or_else(|e| {
                                        error!("list:load error flow_id={flow_id} err={:?}", e);
                                        panic!("diesel load error: {e}");
                                    });
        let events: Vec<FlowEvent> = rows.into_iter().filter_map(deserialize_full_enum).collect();
        debug!("list:done flow_id={flow_id} count={}", events.len());
        events
    }
}

impl<P: ConnectionProvider> PgEventStore<P> {
    /// Lista errores de ejecución para un flow_id, ordenados por ts.
    pub fn list_errors(&self, flow_id: Uuid) -> Vec<ErrorRow> {
        debug!("list_errors:start flow_id={flow_id}");
        let rows: Vec<ErrorRow> = with_retry(|| {
                                      let mut conn = self.provider.connection()?;
                                      let query =
                                          step_execution_errors::table.filter(step_execution_errors::flow_id.eq(flow_id))
                                                                      .order(step_execution_errors::ts.asc());
                                      query.load(&mut conn).map_err(PersistenceError::from)
                                  }).unwrap_or_else(|e| {
                                        error!("list_errors:load error flow_id={flow_id} err={:?}", e);
                                        vec![]
                                    });
        debug!("list_errors:done flow_id={flow_id} count={}", rows.len());
        rows
    }
}

/// Nombre legible de la variante del evento para logging/diagnóstico.
fn kind_variant_name(kind: &FlowEventKind) -> &'static str {
    match kind {
        FlowEventKind::FlowInitialized { .. } => "FlowInitialized",
        FlowEventKind::StepStarted { .. } => "StepStarted",
        FlowEventKind::StepFinished { .. } => "StepFinished",
        FlowEventKind::StepFailed { .. } => "StepFailed",
        FlowEventKind::StepSignal { .. } => "StepSignal",
        FlowEventKind::PropertyPreferenceAssigned { .. } => "PropertyPreferenceAssigned",
        FlowEventKind::RetryScheduled { .. } => "RetryScheduled",
        FlowEventKind::BranchCreated { .. } => "BranchCreated",
        FlowEventKind::UserInteractionRequested { .. } => "UserInteractionRequested",
        FlowEventKind::UserInteractionProvided { .. } => "UserInteractionProvided",
        FlowEventKind::FlowCompleted { .. } => "FlowCompleted",
    }
}

/// Implementación Postgres de `FlowRepository` delegada a la versión InMemory.
///
/// Decisión de diseño:
/// - Para asegurar paridad exacta con el core y evitar duplicación de reglas,
///   se reutiliza el `InMemoryFlowRepository` para construir el `FlowInstance`
///   a partir de los eventos leídos.
pub struct PgFlowRepository;
impl PgFlowRepository {
    /// Constructor sin estado.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PgFlowRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowRepository for PgFlowRepository {
    fn load(&self, flow_id: Uuid, events: &[FlowEvent], definition: &FlowDefinition) -> FlowInstance {
        InMemoryFlowRepository::new().load(flow_id, events, definition)
    }
}

/// Construye un pool Postgres r2d2 a partir de URL.
///
/// Comportamiento:
/// - Valida y ajusta tamaños (si `min_size > max_size`, usa `min_size =
///   max_size`).
/// - Ejecuta migraciones inmediatamente tras el primer `get()`.
/// - Devuelve `PersistenceError::TransientIo` ante errores del pool/manager.
pub fn build_pool(database_url: &str, min_size: u32, max_size: u32) -> Result<PgPool, PersistenceError> {
    let validated_min = if min_size == 0 { 1 } else { min_size };
    let validated_max = if max_size == 0 { 1 } else { max_size };
    if validated_min > validated_max {
        eprintln!("WARN: min_size > max_size ({} > {}), ajustando min=max",
                  validated_min, validated_max);
    }
    let final_min = validated_min.min(validated_max);
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder().min_idle(Some(final_min))
                                    .max_size(validated_max)
                                    .build(manager)
                                    .map_err(|e| PersistenceError::TransientIo(format!("pool build: {e}")))?;
    // Ejecutar migraciones una sola vez al construir (primer connection checkout).
    {
        let mut conn = pool.get()
                           .map_err(|e| PersistenceError::TransientIo(format!("pool get for migrations: {e}")))?;
        run_pending_migrations(&mut conn)?;
    }
    Ok(pool)
}

/// Alias explícito para semántica clara (igual a `build_pool` actualmente).
pub fn build_pool_with_migrations(database_url: &str, min: u32, max: u32) -> Result<PgPool, PersistenceError> {
    build_pool(database_url, min, max)
}

/// Helper de desarrollo: carga `.env`, lee configuración (DATABASE_URL,
/// tamaños) y construye un pool ya migrado.
pub fn build_dev_pool_from_env() -> Result<PgPool, PersistenceError> {
    crate::config::init_dotenv();
    let cfg = crate::config::DbConfig::from_env();
    build_pool(&cfg.url, cfg.min_connections, cfg.max_connections)
}
