-- 0005: Tabla para persistir errores de ejecución de steps con retry_count
-- Soporte para auditoría granular de fallos y reconstrucción de timeline.

CREATE TABLE IF NOT EXISTS step_execution_errors (
    id BIGSERIAL PRIMARY KEY,
    flow_id UUID NOT NULL,
    step_id TEXT NOT NULL,
    attempt_number INT NOT NULL CHECK (attempt_number >= 0),
    error_class TEXT NOT NULL CHECK (error_class IN ('validation', 'runtime', 'transient', 'permanent')),
    details JSONB,
    ts TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Índices para consultas eficientes
CREATE INDEX IF NOT EXISTS idx_step_execution_errors_flow_id ON step_execution_errors(flow_id);
CREATE INDEX IF NOT EXISTS idx_step_execution_errors_step_attempt ON step_execution_errors(step_id, attempt_number);
