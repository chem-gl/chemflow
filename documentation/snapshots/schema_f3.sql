-- Placeholder: se actualizará con dump pg_dump o reconstrucción manual del DDL vigente.
-- Versión inicial coincide con migraciones 0001 y 0002.
CREATE TABLE event_log (
    seq BIGSERIAL PRIMARY KEY,
    flow_id UUID NOT NULL,
    ts TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_type TEXT NOT NULL CHECK (event_type = lower(event_type)) CHECK (event_type IN (
        'flowinitialized','stepstarted','stepfinished','stepfailed','stepsignal','flowcompleted'
    )),
    payload JSONB NOT NULL
);
CREATE INDEX idx_event_log_flow_seq ON event_log(flow_id, seq);
CREATE TABLE workflow_step_artifacts (
    artifact_hash TEXT PRIMARY KEY CHECK (length(artifact_hash)=64),
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    metadata JSONB NULL,
    produced_in_seq BIGINT NOT NULL REFERENCES event_log(seq) ON DELETE RESTRICT
);
CREATE INDEX idx_artifacts_seq ON workflow_step_artifacts(produced_in_seq);
