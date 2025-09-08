# chem-persistence (F5 – Persistencia Postgres mínima + F8 – Errores Extendidos)

Propósito: lograr durabilidad en Postgres con paridad 1:1 frente al backend en memoria, manteniendo el determinismo del motor y aislando los mapeos dominio↔filas. F8 añade persistencia granular de errores para auditoría y reconstrucción de timeline.

## Capas y módulos

- `pg/` – Implementaciones Diesel:
  - `PgEventStore`: append-only de `event_log` + inserción opcional de `workflow_step_artifacts` atómica con `StepFinished` + inserción de `step_execution_errors` atómica con `StepFailed` (F8).
  - `PgFlowRepository`: delega el replay a `InMemoryFlowRepository` (paridad exacta).
- `migrations/` – Migraciones Diesel transaccionales e idempotentes (incluye 0005_step_execution_errors para F8).
- `config/` – Carga de `DATABASE_URL` y parámetros de pool.
- `schema/` – Esquema Diesel (event_log, workflow_step_artifacts, step_execution_errors).

## Esquema

- `event_log(seq BIGSERIAL PK, flow_id UUID, ts timestamptz default now(), event_type text CHECK lower(), payload jsonb)`
  - Índice: `(flow_id, seq)`
- `workflow_step_artifacts(artifact_hash TEXT PK len=64, kind text, payload jsonb, metadata jsonb null, produced_in_seq bigint FK event_log(seq))`
  - Índice: `(produced_in_seq)`
- `step_execution_errors(id BIGSERIAL PK, flow_id UUID, step_id text, attempt_number int CHECK >=0, error_class text CHECK IN ('validation','runtime','transient','permanent'), details jsonb null, ts timestamptz default now())` (F8)
  - Índices: `(flow_id)`, `(step_id, attempt_number)`

Notas:

- `event_type` debe ser minúsculas y pertenece al conjunto {flowinitialized, stepstarted, stepfinished, stepfailed, stepsignal, propertypreferenceassigned, retryscheduled, flowcompleted}. (Añadido en migraciones 0003 y 0004)
- `seq` es append-only y global a la tabla.
- `step_execution_errors` se inserta automáticamente al emitir `StepFailed`, clasificando el error y guardando detalles JSON.

## Comportamiento clave

- Serialización estable: `FlowEventKind` completo en `payload` (JSONB). `event_type` sólo como pista y constraint.
- Atomicidad: si `StepFinished` incluye `outputs`, se inserta cada `artifact_hash` en `workflow_step_artifacts` dentro de la MISMA transacción que el evento. Si el feature `no-artifact-insert` está activo, se omite. (F8) Para `StepFailed`, se inserta en `step_execution_errors` en la misma transacción.
- Retry/backoff: reintentos para `append` y `list` ante conflictos de serialización, IO transitorios y ciertos mensajes de desconexión/timeout.
- Paridad: `PgFlowRepository` usa la lógica in-memory para `load`.
- Clasificación de errores (F8): `Internal` y `StorageError` → 'runtime'; otros → 'validation'. `attempt_number` simplificado a 1 (futuro: contar `StepStarted`).

## Configuración

Variables:

- `DATABASE_URL` – Postgres accesible (p.ej. `postgres://user:pass@localhost:5432/chem`)
- `DATABASE_MIN_CONNECTIONS` – min idle (por defecto 2)
- `DATABASE_MAX_CONNECTIONS` – tamaño máximo (por defecto 16)

Arranque local rápido:

1. Inicia Postgres (puedes usar `postgress-docker/compose.yaml` del repo raíz).
2. Exporta `DATABASE_URL`.
3. Ejecuta tests del crate: `cargo test -p chem-persistence`.

## Tests incluidos

- `event_parity.rs` – Paridad de secuencia y tipos frente a InMemory.
- `event_roundtrip_variants.rs` – Roundtrip de todas las variantes del enum vía JSON.
- `seq_integrity.rs` – Contigüidad relativa de `seq` para un `flow_id`.
- `engine_fingerprint.rs` – Fingerprint final coincide entre PG e InMemory.
- `event_type_constraint.rs` – Constraint de tipos reacciona ante valores inválidos.
- `stress.rs` – Inserciones masivas con y sin artifacts.
- `minimal_pool.rs`, `teardown.rs` – Diagnóstico de pool/conexión.
- `error_persistence.rs` (F8) – Inserción automática de errores, clasificación y consulta.

Para ejecutar sólo este paquete:

```bash
cargo test -p chem-persistence
```

## Operabilidad

- Migraciones: se ejecutan automáticamente al construir el pool (la primera conexión corre `run_pending_migrations`).
- Backup/restore: usa herramientas de Postgres (pg_dump/pg_restore). El log es append-only; evita `UPDATE/DELETE` en `event_log`.
- Índices: `(flow_id, seq)` obligatorio. Secundarios adicionales diferidos.
- Timeline de errores (F8): usa `PgEventStore::list_errors(flow_id)` para reconstruir fallos por step y attempt.

## Flags/Features

- `no-artifact-insert`: desactiva inserción de `workflow_step_artifacts` (útil para aislar problemas durante depuración o cuando aún no se usan artifacts persistidos).

## Futuro cercano (deferido)

- Guardar `kind`, `payload` y `metadata` reales de artifacts.
- Índices parciales por `event_type` y consultas analíticas.
- Métricas ligeras y tracing opcional.
- attempt_number preciso (contar eventos previos).
