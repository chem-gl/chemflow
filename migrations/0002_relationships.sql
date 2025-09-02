-- 0002_relationships.sql
-- Link table between workflow step executions and molecule families (many-to-many potential)

CREATE TABLE IF NOT EXISTS workflow_step_family (
    step_id UUID NOT NULL REFERENCES workflow_step_executions(step_id) ON DELETE CASCADE,
    family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE,
    PRIMARY KEY (step_id, family_id)
);
