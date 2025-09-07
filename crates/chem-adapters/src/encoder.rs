//! Encoder Dominio → Artifact neutral (F4)
//!
//! Este módulo define `DomainArtifactEncoder`, un trait para empaquetar
//! entidades de dominio (`chem-domain`) en artifacts neutrales (`chem-core`).
//!
//! Reglas clave:
//! - El `payload` debe ser JSON canónico y estable (orden determinista de
//!   campos y colecciones) para que el hash calculado por el engine sea
//!   reproducible.
//! - El `kind` será `ArtifactKind::GenericJson` en F4 (no extendemos el enum
//!   del core en esta fase; la distinción se hace por el shape del payload).
//! - Este encoder NO calcula `hash` (lo hace el engine cuando acepta outputs).

use chem_core::model::{Artifact, ArtifactSpec};
use serde_json::json;

use chem_domain::{MolecularProperty, Molecule, MoleculeFamily};

/// Contrato de empaquetado dominio → artifact neutral.
pub trait DomainArtifactEncoder {
    /// Empaqueta una molécula a artifact neutro.
    fn encode_molecule(&self, m: &Molecule) -> Artifact;
    /// Empaqueta una familia a artifact `FamilyArtifact`-like.
    fn encode_family(&self, f: &MoleculeFamily) -> Artifact;
    /// Empaqueta una propiedad molecular a artifact neutro.
    fn encode_property<'a, V, M>(&self, p: &MolecularProperty<'a, V, M>) -> Artifact
        where V: serde::Serialize + Clone,
              M: serde::Serialize + Clone;
}

/// Implementación simple de `DomainArtifactEncoder`.
///
/// Mantiene un esquema mínimo de payloads:
/// - Molecule: { inchikey, smiles, inchi }
/// - Family: { family_hash, ordered_keys: [...] }
/// - Property: { molecule_inchikey, property_kind, value, units? }
#[derive(Clone, Default)]
pub struct SimpleDomainEncoder;

impl DomainArtifactEncoder for SimpleDomainEncoder {
    fn encode_molecule(&self, m: &Molecule) -> Artifact {
        // Usar artifact tipado para evitar constructor privado del core.
        // El macro inserta schema_version y el engine añadirá el hash.
        let typed = crate::artifacts::MoleculeArtifact { inchikey: m.inchikey().to_string(),
                                                         smiles: m.smiles().to_string(),
                                                         inchi: m.inchi().to_string(),
                                                         schema_version: 1 };
        typed.into_artifact()
    }

    fn encode_family(&self, f: &MoleculeFamily) -> Artifact {
        let ordered_keys: Vec<String> = f.molecules().iter().map(|m| m.inchikey().to_string()).collect();
        let typed = crate::artifacts::FamilyArtifact { family_hash: f.family_hash().to_string(),
                                                       ordered_keys,
                                                       schema_version: 1 };
        typed.into_artifact()
    }

    fn encode_property<'a, V, M>(&self, p: &MolecularProperty<'a, V, M>) -> Artifact
        where V: serde::Serialize + Clone,
              M: serde::Serialize + Clone
    {
        let typed = crate::artifacts::MolecularPropertyArtifact { molecule_inchikey: p.molecule().inchikey().to_string(),
                                                                  property_kind: p.property_type().to_string(),
                                                                  value: json!(p.value()),
                                                                  units: None,
                                                                  schema_version: 1 };
        typed.into_artifact()
    }
}
