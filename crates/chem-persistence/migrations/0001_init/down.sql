-- 0001_init/down.sql

DROP INDEX IF EXISTS ix_branches_parent;
DROP INDEX IF EXISTS ix_branches_root;
DROP TABLE IF EXISTS workflow_branches;

DROP INDEX IF EXISTS idx_step_execution_errors_step_attempt;
DROP INDEX IF EXISTS idx_step_execution_errors_flow_id;
DROP TABLE IF EXISTS step_execution_errors;

DROP INDEX IF EXISTS idx_artifacts_seq;
DROP TABLE IF EXISTS workflow_step_artifacts;

DROP INDEX IF EXISTS idx_event_log_flow_seq;
ALTER TABLE IF EXISTS event_log DROP CONSTRAINT IF EXISTS event_log_event_type_check;
DROP TABLE IF EXISTS event_log;

-- Note: do not drop global extensions like pgcrypto here.
-- 0005_step_execution_errors/down.sql
-- Down migration for 0005: remove the step_execution_errors table and indexes

DROP INDEX IF EXISTS idx_step_execution_errors_step_attempt;
DROP INDEX IF EXISTS idx_step_execution_errors_flow_id;
DROP TABLE IF EXISTS step_execution_errors;

-- Note: do not drop global extensions or other unrelated objects.
