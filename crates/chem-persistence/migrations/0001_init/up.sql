
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TABLE IF NOT EXISTS event_log (
    seq BIGSERIAL PRIMARY KEY,
    flow_id UUID NOT NULL,
    ts TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL
);
ALTER TABLE event_log
    DROP CONSTRAINT IF EXISTS event_log_event_type_check;
ALTER TABLE event_log
    DROP CONSTRAINT IF EXISTS event_log_event_type_in_check;
ALTER TABLE event_log
    DROP CONSTRAINT IF EXISTS event_log_event_type_check1;

ALTER TABLE event_log
    ADD CONSTRAINT event_log_event_type_check
    CHECK (
        event_type = lower(event_type)
        AND event_type IN (
            'flowinitialized',
            'stepstarted',
            'stepfinished',
            'stepfailed',
            'stepsignal',
            'propertypreferenceassigned',
            'retryscheduled',
            'branchcreated',
            'userinteractionrequested',
            'userinteractionprovided',
            'flowcompleted'
        )
    );

CREATE INDEX IF NOT EXISTS idx_event_log_flow_seq ON event_log(flow_id, seq);

-- Artifacts produced by steps (optional in early phases)
CREATE TABLE IF NOT EXISTS workflow_step_artifacts (
    artifact_hash TEXT PRIMARY KEY CHECK (length(artifact_hash) = 64),
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    metadata JSONB NULL,
    produced_in_seq BIGINT NOT NULL REFERENCES event_log(seq) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_artifacts_seq ON workflow_step_artifacts(produced_in_seq);

-- Persisted step execution errors para auditoría (F8).
-- Definimos una única tabla canonica `step_execution_errors`.
CREATE TABLE IF NOT EXISTS step_execution_errors (
    id BIGSERIAL PRIMARY KEY,
    flow_id UUID NOT NULL,
    step_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL CHECK (attempt_number >= 0),
    -- Clase de error para categorizar (validation/runtime/transient/permanent)
    error_class TEXT NOT NULL CHECK (error_class IN ('validation','runtime','transient','permanent')),
    details JSONB NULL,
    ts TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_step_execution_errors_flow_id ON step_execution_errors(flow_id);
CREATE INDEX IF NOT EXISTS idx_step_execution_errors_step_attempt ON step_execution_errors(step_id, attempt_number);
