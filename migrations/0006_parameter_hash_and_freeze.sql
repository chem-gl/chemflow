-- 0006_parameter_hash_and_freeze.sql
ALTER TABLE workflow_step_executions
    ADD COLUMN IF NOT EXISTS parameter_hash TEXT;

ALTER TABLE molecule_families
    ADD COLUMN IF NOT EXISTS frozen BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS frozen_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS family_hash TEXT;

CREATE INDEX IF NOT EXISTS idx_workflow_step_parameter_hash ON workflow_step_executions(parameter_hash);
