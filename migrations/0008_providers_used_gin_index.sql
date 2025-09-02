-- Crea índice GIN sobre providers_used para acelerar consultas por campos internos (JSONB)
CREATE INDEX IF NOT EXISTS idx_workflow_step_executions_providers_used_gin
    ON workflow_step_executions USING GIN (providers_used jsonb_path_ops);
