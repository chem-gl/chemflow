-- Snapshot de esquema F6 (incluye evento de políticas)
-- La fuente de verdad son las migraciones (0001..0003).

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Tabla de eventos append-only
CREATE TABLE IF NOT EXISTS event_log (
    seq BIGSERIAL PRIMARY KEY,
    flow_id UUID NOT NULL,
    ts TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_type TEXT NOT NULL CHECK (event_type = lower(event_type)) CHECK (event_type IN (
        'flowinitialized','stepstarted','stepfinished','stepfailed','stepsignal','propertypreferenceassigned','flowcompleted'
    )),
    payload JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_event_log_flow_seq ON event_log(flow_id, seq);

-- Tabla opcional para deduplicación de artifacts por hash
CREATE TABLE IF NOT EXISTS workflow_step_artifacts (
    artifact_hash TEXT PRIMARY KEY CHECK (length(artifact_hash)=64),
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    metadata JSONB NULL,
    produced_in_seq BIGINT NOT NULL REFERENCES event_log(seq) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_artifacts_seq ON workflow_step_artifacts(produced_in_seq);
