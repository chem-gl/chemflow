-- 0004_molecules.sql
-- Normalización de moléculas: crear tabla independiente y relación N:M con familias.
-- Cada molécula se almacena una sola vez (inchikey = PK). Las familias ahora
-- referencian moléculas mediante la tabla de unión `molecule_family_molecules`.
-- La columna existente `molecule_families.molecules` pasa a almacenar únicamente
-- un arreglo JSON de InChIKeys (para compatibilidad retro) y puede ser eliminada
-- en una migración futura.
CREATE TABLE IF NOT EXISTS molecules (
    inchikey TEXT PRIMARY KEY,
    inchi TEXT NOT NULL,
    smiles TEXT NOT NULL,
    common_name TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Trigger simple (opcional) para updated_at. Si no se desea lógica PL/pgSQL puede omitirse.
-- Aquí se deja preparado por si se activa posteriormente.
-- CREATE OR REPLACE FUNCTION set_updated_at()
-- RETURNS TRIGGER AS $$
-- BEGIN
--     NEW.updated_at = now();
--     RETURN NEW;
-- END;
-- $$ LANGUAGE plpgsql;
-- CREATE TRIGGER trg_molecules_updated
-- BEFORE UPDATE ON molecules
-- FOR EACH ROW EXECUTE PROCEDURE set_updated_at();
CREATE TABLE IF NOT EXISTS molecule_family_molecules (
    family_id UUID NOT NULL REFERENCES molecule_families(id) ON DELETE CASCADE,
    molecule_inchikey TEXT NOT NULL REFERENCES molecules(inchikey) ON DELETE CASCADE,
    position INT NOT NULL DEFAULT 0,
    PRIMARY KEY (family_id, molecule_inchikey)
);
CREATE INDEX IF NOT EXISTS idx_molecule_family_molecules_family ON molecule_family_molecules(family_id);
CREATE INDEX IF NOT EXISTS idx_molecule_family_molecules_inchikey ON molecule_family_molecules(molecule_inchikey);
