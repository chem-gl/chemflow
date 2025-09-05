//! Implementaciones Postgres (Diesel) de los traits del core.
//! F3: EventStore append-only y FlowRepository (replay delegada) equivalentes a memoria.

use chem_core::repo::FlowInstance;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde_json::Value;

use chem_core::{EventStore, FlowEvent, FlowEventKind, FlowRepository, FlowDefinition, InMemoryFlowRepository};
use log::{debug, error};

use crate::error::PersistenceError;
use crate::migrations::run_pending_migrations;
use crate::schema::{event_log, workflow_step_artifacts};

pub type PgPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Trait interno para obtener una conexión (para testear fácilmente).
pub trait ConnectionProvider: Send + Sync + 'static {
    fn connection(&self) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, PersistenceError>;
}

/// Implementación de provider a partir de un pool r2d2.
pub struct PoolProvider { pub pool: PgPool }
impl ConnectionProvider for PoolProvider {
    fn connection(&self) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, PersistenceError> {
        self.pool.get()
            .map_err(|e| PersistenceError::TransientIo(format!("pool error: {e}")))
    }
}

/// Row mapeada de la tabla `event_log` (shape mínima anticipada).
#[derive(Queryable, Debug)]
pub struct EventRow { pub seq: i64, pub flow_id: uuid::Uuid, pub ts: DateTime<Utc>, pub event_type: String, pub payload: Value }

/// Estructura para inserción (NewEventRow) - `RETURNING` seq, ts.
#[derive(Insertable, Debug)]
#[diesel(table_name = event_log)]
pub struct NewEventRow<'a> { pub flow_id: &'a uuid::Uuid, pub event_type: &'a str, pub payload: &'a Value }

/// Fila para insertar artifact (deduplicación por hash via ON CONFLICT DO NOTHING lógica manual).
#[derive(Insertable, Debug)]
#[diesel(table_name = workflow_step_artifacts)]
pub struct NewArtifactRow<'a> { pub artifact_hash: &'a str, pub kind: &'a str, pub payload: &'a Value, pub metadata: Option<&'a Value>, pub produced_in_seq: i64 }

// Retry simple para conflictos de serialización.
fn with_retry<F, T>(mut f: F) -> Result<T, PersistenceError>
where F: FnMut() -> Result<T, PersistenceError> {
    let mut attempts = 0;
    loop {
        match f() {
            Err(PersistenceError::SerializationConflict) if attempts < 3 => {
                std::thread::sleep(std::time::Duration::from_millis(15 * (attempts + 1) as u64));
                attempts += 1;
            }
            r => return r,
        }
    }
}

// SERIALIZACIÓN: guardamos el enum completo como JSON, pero usamos event_type = variante en minúsculas
// para respetar el constraint existente en la migración.
fn serialize_full_enum(kind: &FlowEventKind) -> Value { serde_json::to_value(kind).expect("serialize FlowEventKind") }

fn event_type_for(kind: &FlowEventKind) -> &'static str {
    match kind {
        FlowEventKind::FlowInitialized { .. } => "flowinitialized",
        FlowEventKind::StepStarted { .. } => "stepstarted",
        FlowEventKind::StepFinished { .. } => "stepfinished",
        FlowEventKind::StepFailed { .. } => "stepfailed",
        FlowEventKind::StepSignal { .. } => "stepsignal",
        FlowEventKind::FlowCompleted { .. } => "flowcompleted",
    }
}

fn deserialize_full_enum(row: EventRow) -> Option<FlowEvent> {
    // Aceptamos cualquiera de los tipos válidos; payload siempre es JSON del enum completo.
    let kind: FlowEventKind = serde_json::from_value(row.payload).ok()?;
    Some(FlowEvent { seq: row.seq as u64, flow_id: row.flow_id, kind, ts: row.ts })
}

/// Implementación Postgres de EventStore.
pub struct PgEventStore<P: ConnectionProvider> { pub provider: P }
impl<P: ConnectionProvider> PgEventStore<P> {
    pub fn new(provider: P) -> Self { Self { provider } }
}

impl<P: ConnectionProvider> EventStore for PgEventStore<P> {
    fn append_kind(&mut self, flow_id: Uuid, kind: FlowEventKind) -> FlowEvent {
        debug!("append_kind:start flow_id={flow_id} kind={}", kind_variant_name(&kind));
        let event_type = event_type_for(&kind);
        let payload = serialize_full_enum(&kind);

        // Ejecutamos (insert evento + artifacts) dentro de una sola transacción retryable.
        // Paso 1: insertar sólo el evento (transacción mínima) para garantizar commit aunque fallen artifacts.
        let inserted: (i64, DateTime<Utc>) = with_retry(|| {
            let mut conn = self.provider.connection()?;
            conn.build_transaction().read_write().run(|tx_conn| {
                diesel::insert_into(event_log::table)
                    .values(NewEventRow { flow_id: &flow_id, event_type, payload: &payload })
                    .returning((event_log::seq, event_log::ts))
                    .get_result(tx_conn)
            }).map_err(PersistenceError::from)
        }).expect("insert event");

        // Paso 2: insertar artifacts (best-effort) fuera de la transacción del evento.
        #[cfg(not(feature = "no-artifact-insert"))]
        {
            if let FlowEventKind::StepFinished { outputs, .. } = &kind {
                if !outputs.is_empty() {
                    match self.provider.connection() {
                        Ok(mut conn2) => {
                            for h in outputs {
                                if h.len() != 64 { debug!("skip artifact hash len!=64 hash={h}"); continue; }
                                let null = Value::Null;
                                let row = NewArtifactRow { artifact_hash: h, kind: "unknown", payload: &null, metadata: None, produced_in_seq: inserted.0 };
                                if let Err(e) = diesel::insert_into(workflow_step_artifacts::table)
                                    .values(&row)
                                    .on_conflict_do_nothing()
                                    .execute(&mut conn2) {
                                    error!("artifact insert error hash={h} seq={} err={:?}", inserted.0, e);
                                }
                            }
                        }
                        Err(e) => error!("artifact connection error seq={} err={:?}", inserted.0, e),
                    }
                }
            }
        }
        #[cfg(feature = "no-artifact-insert")]
        {
            if let FlowEventKind::StepFinished { outputs, .. } = &kind { debug!("artifact insertion skipped by feature no-artifact-insert outputs={}", outputs.len()); }
        }

        let ev = FlowEvent { seq: inserted.0 as u64, flow_id, kind, ts: inserted.1 };
        debug!("append_kind:done flow_id={flow_id} seq={} kind={}", ev.seq, kind_variant_name(&ev.kind));
        ev
    }
    fn list(&self, flow_id: Uuid) -> Vec<FlowEvent> {
        debug!("list:start flow_id={flow_id}");
        let mut conn = self.provider.connection().expect("conn");
        let query = event_log::table
            .filter(event_log::flow_id.eq(flow_id))
            .order(event_log::seq.asc());
        let rows: Vec<EventRow> = match query.load(&mut conn) {
            Ok(r) => r,
            Err(e) => {
                error!("list:load error flow_id={flow_id} err={:?}", e);
                panic!("diesel load error: {e}");
            }
        };
        let events: Vec<FlowEvent> = rows.into_iter().filter_map(deserialize_full_enum).collect();
        debug!("list:done flow_id={flow_id} count={}", events.len());
        events
    }
}

fn kind_variant_name(kind: &FlowEventKind) -> &'static str {
    match kind {
        FlowEventKind::FlowInitialized { .. } => "FlowInitialized",
        FlowEventKind::StepStarted { .. } => "StepStarted",
        FlowEventKind::StepFinished { .. } => "StepFinished",
        FlowEventKind::StepFailed { .. } => "StepFailed",
        FlowEventKind::StepSignal { .. } => "StepSignal",
        FlowEventKind::FlowCompleted { .. } => "FlowCompleted",
    }
}

/// Implementación Postgres de FlowRepository (delegate a replay in-memory simple).
pub struct PgFlowRepository;
impl PgFlowRepository { pub fn new() -> Self { Self } }

impl FlowRepository for PgFlowRepository {
    fn load(&self, flow_id: Uuid, events: &[FlowEvent], definition: &FlowDefinition) -> FlowInstance {
        InMemoryFlowRepository::new().load(flow_id, events, definition)
    }
}

/// Construye un pool Postgres r2d2 a partir de URL.
pub fn build_pool(database_url: &str, min_size: u32, max_size: u32) -> Result<PgPool, PersistenceError> {
    let validated_min = if min_size == 0 { 1 } else { min_size };
    let validated_max = if max_size == 0 { 1 } else { max_size };
    if validated_min > validated_max { eprintln!("WARN: min_size > max_size ({} > {}), ajustando min=max", validated_min, validated_max); }
    let final_min = validated_min.min(validated_max);
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .min_idle(Some(final_min))
        .max_size(validated_max)
        .build(manager)
        .map_err(|e| PersistenceError::TransientIo(format!("pool build: {e}")))?;
    // Ejecutar migraciones una sola vez al construir (primer connection checkout).
    {
        let mut conn = pool.get().map_err(|e| PersistenceError::TransientIo(format!("pool get for migrations: {e}")))?;
        run_pending_migrations(&mut conn)?;
    }
    Ok(pool)
}

/// Alias explícito para semántica clara (igual a build_pool actualmente).
pub fn build_pool_with_migrations(database_url: &str, min: u32, max: u32) -> Result<PgPool, PersistenceError> {
    build_pool(database_url, min, max)
}

/// Helper conveniente para desarrollo: carga .env, lee configuración y construye pool (con migraciones).
pub fn build_dev_pool_from_env() -> Result<PgPool, PersistenceError> {
    crate::config::init_dotenv();
    let cfg = crate::config::DbConfig::from_env();
    build_pool(&cfg.url, cfg.min_connections, cfg.max_connections)
}
