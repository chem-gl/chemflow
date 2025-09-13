//! AcquireMoleculesStep (Source determinista)
//!
//! - Emite un único artifact de familia (`FamilyArtifact`) derivado de un
//!   dataset sintético y determinista.
//! - No accede a IO externo; sólo crea estructuras en memoria.
//! - El motor calculará el hash del artifact a partir del payload canónico.

use chem_core::typed_step;

use crate::artifacts::FamilyArtifact;
use chem_domain::{DomainError, Molecule, MoleculeFamily};

/// Parámetros del step. En F4 mantenemos un solo dataset sintético.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AcquireParams {
    /// Nombre del dataset sintético. Por defecto: "synthetic_v1".
    pub dataset: String,
}

/// Construye una familia determinista a partir de un dataset sintético.
fn build_synthetic_family(dataset: &str) -> Result<MoleculeFamily, DomainError> {
    // Elegimos SMILES simples y estables; RDKit/chemengine generará inchikeys.
    // Nota: Evitar cambios de orden o contenido para preservar determinismo.
    let smiles_list: &[&str] = match dataset {
        "synthetic_v1" | "default" | "" => &["C1=CC=CC=C1", // Benzene
                                             "CCO",         // Ethanol
                                             "CC(=O)O"      /* Acetic acid */],
        other => {
            // Para datasets no reconocidos, usamos el default pero dejamos trazabilidad.
            let _ = other; // reservado para futura extensión
            &["C1=CC=CC=C1", "CCO", "CC(=O)O"]
        }
    };

    let mut mols = Vec::with_capacity(smiles_list.len());
    for s in smiles_list {
        let m = Molecule::from_smiles(s)?;
        mols.push(m);
    }
    // provenance mínimo y estable
    let provenance = serde_json::json!({
        "source": "synthetic",
        "dataset": dataset,
        "version": 1,
    });
    MoleculeFamily::new(mols, provenance)
}

// Step tipado (Source): sin input, output = FamilyArtifact, params =
// AcquireParams.
typed_step! {
    source AcquireMoleculesStep {
        id: "acquire_molecules",
        output: FamilyArtifact,
        params: AcquireParams,
        run(_me, p) {{
            // Construcción determinista de la familia.
            let fam = build_synthetic_family(&p.dataset).expect("synthetic family build");
            // Convertir a artifact tipado (payload estable). El engine añadirá hash.
            FamilyArtifact { family_hash: fam.family_hash().to_string(),
                             ordered_keys: fam.molecules().iter().map(|m| m.inchikey().to_string()).collect(),
                             schema_version: 1 }
        }}
    }
}

// El macro `typed_step!` genera un struct unitario `AcquireMoleculesStep`.
// El builder del engine requiere que los tipos de step implementen `Debug`.
// Ahora el macro incluye Debug automáticamente, por lo que no necesitamos
// implementación manual.
