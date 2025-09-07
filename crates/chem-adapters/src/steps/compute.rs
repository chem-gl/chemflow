//! ComputePropertiesStep (Transform stub, sin selección)
//!
//! - Recibe un `FamilyArtifact` y produce un `FamilyPropertiesArtifact` que
//!   contiene un item por cada molécula (N in == N out por invariantes F4).
//! - Los valores son deterministas (stub): `score = len(inchikey)` y `units =
//!   "au"`.

use chem_core::{step::StepKind, typed_step};

use crate::artifacts::{FamilyArtifact, FamilyPropertiesArtifact, PropertyItem};

/// Parámetros del step compute; dejamos espacio para variantes futuras
/// (p. ej. diferentes stubs): kind = "stub_v1" por defecto.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ComputeParams {
    pub kind: String,
}

typed_step! {
    step ComputePropertiesStep {
        id: "compute_properties",
        kind: StepKind::Transform,
        input: FamilyArtifact,
        output: FamilyPropertiesArtifact,
        params: ComputeParams,
        run(_self, inp, p) {{
            let _stub_kind = if p.kind.is_empty() { "stub_v1" } else { p.kind.as_str() };
            // Generar exactamente un item por ordered key del input, sin filtrado.
            let items: Vec<PropertyItem> = inp.ordered_keys
                                             .iter()
                                             .map(|k| PropertyItem { molecule_inchikey: k.clone(),
                                                                      property_kind: "StubScore".to_string(),
                                                                      value: serde_json::json!({ "score": k.len() }),
                                                                      units: Some("au".to_string()) })
                                             .collect();
            FamilyPropertiesArtifact { family_hash: inp.family_hash,
                                       items,
                                       schema_version: 1 }
        }}
    }
}
