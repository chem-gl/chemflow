-- Migration 0006: Create WORKFLOW_BRANCHES table for branching metadata
CREATE TABLE IF NOT EXISTS workflow_branches (
  branch_id UUID PRIMARY KEY,
  root_flow_id UUID NOT NULL,
  parent_flow_id UUID NULL,
  created_from_step_id TEXT NOT NULL,
  divergence_params_hash TEXT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  name TEXT NULL,
  metadata JSONB NULL
);

CREATE INDEX IF NOT EXISTS ix_branches_root ON workflow_branches(root_flow_id);
CREATE INDEX IF NOT EXISTS ix_branches_parent ON workflow_branches(parent_flow_id);
