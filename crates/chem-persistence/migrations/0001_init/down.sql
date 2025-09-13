
DROP INDEX IF EXISTS idx_step_execution_errors_step_attempt;
DROP INDEX IF EXISTS idx_step_execution_errors_flow_id;
DROP TABLE IF EXISTS step_execution_errors;

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

DROP INDEX IF EXISTS idx_step_execution_errors_step_attempt;
DROP INDEX IF EXISTS idx_step_execution_errors_flow_id;
DROP TABLE IF EXISTS step_execution_errors;
 