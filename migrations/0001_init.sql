-- 0001_init.sql
-- Create tables for workflow executions and molecule families
CREATE TABLE IF NOT EXISTS workflow_step_executions (
    step_id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    providers_used JSONB NOT NULL DEFAULT '[]'::jsonb,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL
);
CREATE TABLE IF NOT EXISTS molecule_families (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    molecules JSONB NOT NULL DEFAULT '[]'::jsonb,
    properties JSONB NOT NULL DEFAULT '{}'::jsonb,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    source_provider JSONB
);
-- Indexes for querying by status and time
CREATE INDEX IF NOT EXISTS idx_workflow_step_status ON workflow_step_executions(status);
CREATE INDEX IF NOT EXISTS idx_workflow_step_start_time ON workflow_step_executions(start_time);
