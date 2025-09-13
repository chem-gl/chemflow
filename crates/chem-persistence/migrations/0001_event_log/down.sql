-- Down migration for 0001_event_log: drop created objects in reverse order
-- This file is intended to be run by Diesel's migration harness when rolling back.

DROP INDEX IF EXISTS ix_branches_parent;
DROP INDEX IF EXISTS ix_branches_root;
DROP TABLE IF EXISTS workflow_branches;

DROP INDEX IF EXISTS idx_step_execution_errors_step_attempt;
DROP INDEX IF EXISTS idx_step_execution_errors_flow_id;
DROP TABLE IF EXISTS step_execution_errors;

DROP INDEX IF EXISTS idx_artifacts_seq;
DROP TABLE IF EXISTS workflow_step_artifacts;

DROP INDEX IF EXISTS idx_event_log_flow_seq;
DROP TABLE IF EXISTS event_log;

-- Note: extension pgcrypto left intact; migrations shouldn't drop global extensions.
