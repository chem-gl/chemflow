-- Migration 0010: Deprecate legacy source_provider column and introduce provenance JSONB column
-- Adds provenance column, migrates existing data, recreates missing normalized tables (idempotent), then drops source_provider.
BEGIN;
-- 1. Add new provenance column if not exists
ALTER TABLE molecule_families ADD COLUMN IF NOT EXISTS provenance JSONB;
-- 2. Migrate data from legacy source_provider into provenance (wrap into object structure) when provenance is NULL
UPDATE molecule_families
SET provenance = jsonb_build_object(
    'created_in_step', NULL,
    'creation_provider', source_provider
)
WHERE provenance IS NULL AND source_provider IS NOT NULL;
-- 3. Create normalized provenance tables if they do not exist (mirrors 0009)
CREATE TABLE IF NOT EXISTS molecule_family_property_providers (
    family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE,
    property_name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    execution_parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    execution_id UUID NOT NULL,
    PRIMARY KEY (family_id, property_name, execution_id)
);
CREATE TABLE IF NOT EXISTS molecule_family_property_steps (
    family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE,
    property_name TEXT NOT NULL,
    step_id UUID NOT NULL,
    PRIMARY KEY (family_id, property_name, step_id)
);
-- 4. Drop legacy column (only if exists and after data migration)
ALTER TABLE molecule_families DROP COLUMN IF EXISTS source_provider;
COMMIT;
