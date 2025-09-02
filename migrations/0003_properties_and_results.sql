-- 0003_properties_and_results.sql
-- Table to store individual property entries per molecule family (flattened for queries)

CREATE TABLE IF NOT EXISTS molecule_family_properties (
    family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE,
    property_name TEXT NOT NULL,
    value DOUBLE PRECISION,
    source TEXT,
    frozen BOOLEAN DEFAULT FALSE,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (family_id, property_name, timestamp)
);

-- Table to store arbitrary step results (JSON)
CREATE TABLE IF NOT EXISTS workflow_step_results (
    step_id UUID NOT NULL REFERENCES workflow_step_executions(step_id) ON DELETE CASCADE,
    result_key TEXT NOT NULL,
    result_value JSONB NOT NULL,
    PRIMARY KEY (step_id, result_key)
);

CREATE INDEX IF NOT EXISTS idx_family_properties_name ON molecule_family_properties(property_name);