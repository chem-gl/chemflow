-- Down migration for 0006_workflow_branches
DROP INDEX IF EXISTS ix_branches_root;
DROP INDEX IF EXISTS ix_branches_parent;
DROP TABLE IF EXISTS workflow_branches;
