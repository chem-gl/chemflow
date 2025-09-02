-- 0007_step_results_type.sql
ALTER TABLE workflow_step_results
    ADD COLUMN IF NOT EXISTS result_type TEXT NOT NULL DEFAULT 'raw';
