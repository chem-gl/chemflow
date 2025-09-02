-- 0009_property_provenance.sql
-- Normalización de la proveniencia de propiedades (proveedores y steps originantes)
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
CREATE INDEX IF NOT EXISTS idx_mf_prop_providers_name ON molecule_family_property_providers(property_name);
CREATE INDEX IF NOT EXISTS idx_mf_prop_steps_name ON molecule_family_property_steps(property_name);