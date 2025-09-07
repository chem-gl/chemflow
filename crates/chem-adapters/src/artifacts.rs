//! Artifacts tipados neutrales usados por los steps de F4.
//!
//! Estos tipos no introducen semántica en el core; sólo definen la forma del
//! `payload` JSON que se serializa a `chem_core::Artifact` con
//! `ArtifactKind::GenericJson` y un `schema_version` estable. Esto permite
//! snapshot tests y estabilidad de hashing (el hash lo calcula el engine a
//! partir del `payload` canónico).

use chem_core::typed_artifact;

// Artifact que representa una molécula individual (neutro para el core).
typed_artifact!(MoleculeArtifact { inchikey: String,
                                   smiles: String,
                                   inchi: String });

// Artifact que representa una familia de moléculas (neutro para el core).
// Campos mínimos y orden determinista:
// - family_hash: hash lógico de la familia desde dominio (ver F1/F3).
// - ordered_keys: lista ordenada y estable de InChIKeys que componen la
//   familia.
// - schema_version: insertado automáticamente por el macro (default=1).
typed_artifact!(FamilyArtifact {
    family_hash: String,
    ordered_keys: Vec<String>,
});

// Ítem de propiedad por molécula incluido dentro de un artifact agregado
// `FamilyPropertiesArtifact` para cumplir el modelo pipeline (un único
// artifact fluye entre steps en F2).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyItem {
    pub molecule_inchikey: String,
    pub property_kind: String,
    pub value: serde_json::Value,
    pub units: Option<String>,
}

// Artifact que agrupa propiedades stub por familia (uno por pipeline).
// - items: exactamente un elemento por `ordered_keys` de la familia de entrada.
typed_artifact!(FamilyPropertiesArtifact {
    family_hash: String,
    items: Vec<PropertyItem>,
});

// Artifact para una propiedad puntual de molécula (cuando se requiera
// itemizar).
typed_artifact!(MolecularPropertyArtifact {
    molecule_inchikey: String,
    property_kind: String,
    value: serde_json::Value,
    units: Option<String>,
});
