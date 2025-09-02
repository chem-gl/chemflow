//! Definición de la entidad `MoleculeFamily` y estructuras relacionadas para
//! representar un conjunto de moléculas y sus propiedades calculadas con
//! trazabilidad completa.
//!
//! Objetivos de este módulo:
//! - Mantener las moléculas agrupadas de forma lógica (familia) para operar en
//!   bloque durante el workflow.
//! - Adjuntar propiedades calculadas (posiblemente múltiples valores) con
//!   referencia explícita al proveedor y parámetros usados.
//! - Facilitar auditoría y branching: cada `FamilyProperty` contiene un
//!   `ProviderReference` que permite saber exactamente el origen de los datos.
//! - Evitar dependencia directa entre propiedades y steps: las propiedades son
//!   independientes del step y sólo registran proveedor + parámetros,
//!   permitiendo reutilización en diferentes workflows / ramas.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::data::types::LogPData;
use crate::molecule::Molecule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoleculeFamily {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub molecules: Vec<Molecule>,
    /// Map property name -> property entry (values + proveedores y steps
    /// históricos)
    pub properties: HashMap<String, FamilyProperty>,
    /// Parámetros asociados (configuración de generación / filtrado)
    pub parameters: HashMap<String, serde_json::Value>,
    /// Proveniencia global (creación inicial)
    pub provenance: Option<FamilyProvenance>,
    /// Flag de congelación lógica
    pub frozen: bool,
    /// Momento de congelación
    pub frozen_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Hash canónico para detectar divergencias
    pub family_hash: Option<String>,
}

/// Represents a property (possibly multi-valued) attached to a family along
/// with the provider & parameters used to calculate it for traceability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyProperty {
    pub values: Vec<LogPData>,
    pub providers: Vec<ProviderReference>,
    pub originating_steps: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReference {
    pub provider_type: String,
    pub provider_name: String,
    pub provider_version: String,
    pub execution_parameters: HashMap<String, serde_json::Value>,
    pub execution_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyProvenance {
    pub created_in_step: Option<Uuid>,
    pub creation_provider: Option<ProviderReference>,
}

impl MoleculeFamily {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self { id: Uuid::new_v4(),
               name,
               description,
               molecules: Vec::new(),
               properties: HashMap::new(),
               parameters: HashMap::new(),
               provenance: None,
               frozen: false,
               frozen_at: None,
               family_hash: None }
    }

    /// Attach a property with complete traceability information.
    pub fn add_property(&mut self, property_name: impl Into<String>, data: Vec<LogPData>, provider_reference: ProviderReference, step_id: Option<Uuid>) {
        let key = property_name.into();
        self.properties
            .entry(key)
            .and_modify(|existing| {
                existing.values.extend(data.clone());
                existing.providers.push(provider_reference.clone());
                if let Some(s) = step_id {
                    existing.originating_steps.push(s);
                }
            })
            .or_insert_with(|| FamilyProperty { values: data,
                                                providers: vec![provider_reference.clone()],
                                                originating_steps: step_id.into_iter().collect() });
        if !self.frozen {
            self.recompute_hash();
        }
    }

    pub fn get_property(&self, property_name: &str) -> Option<&FamilyProperty> {
        self.properties.get(property_name)
    }

    /// Congela la familia: marca `frozen=true`, asigna `frozen_at` y congela
    /// también los valores de propiedades (flag interno) para trazabilidad.
    /// Recalcula y fija `family_hash`.
    pub fn freeze(&mut self) {
        if self.frozen {
            return;
        }
        self.frozen = true;
        self.frozen_at = Some(chrono::Utc::now());
        for prop in self.properties.values_mut() {
            for v in &mut prop.values {
                v.frozen = true;
            }
        }
        self.recompute_hash();
    }
    /// Recalcula el hash canónico de la familia basándose en moléculas,
    /// parámetros y nombres de propiedades.
    pub fn recompute_hash(&mut self) {
        let hash = crate::database::repository::compute_sorted_hash(&serde_json::json!({
                                                                        "id": self.id,
                                                                        "molecules": self.molecules.iter().map(|m| &m.inchikey).collect::<Vec<_>>(),
                                                                        "parameters": self.parameters,
                                                                        "properties": self.properties.keys().collect::<Vec<_>>(),
                                                                        "property_provenance": self.properties.iter().map(|(k,v)| (k, &v.originating_steps)).collect::<Vec<_>>(),
                                                                        "frozen": self.frozen,
                                                                        "frozen_at": self.frozen_at,
                                                                    }));
        self.family_hash = Some(hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::types::LogPData;
    use chrono::Utc;

    #[test]
    fn test_get_property() {
        let mut family = MoleculeFamily::new("Test Family".to_string(), Some("Test".to_string()));
        assert!(!family.frozen);
        assert!(family.family_hash.is_none());

        let logp_data = vec![LogPData { value: 1.5,
                                        source: "test".to_string(),
                                        frozen: false,
                                        timestamp: Utc::now() }];

        let provider_ref = ProviderReference { provider_type: "test".to_string(),
                                               provider_name: "test_provider".to_string(),
                                               provider_version: "1.0".to_string(),
                                               execution_parameters: HashMap::new(),
                                               execution_id: Uuid::new_v4() };

        family.add_property("logp", logp_data.clone(), provider_ref.clone(), Some(Uuid::new_v4()));

        // Test get_property
        let property = family.get_property("logp");
        assert!(property.is_some());
        let p = property.unwrap();
        assert_eq!(p.values.len(), 1);
        assert_eq!(p.values[0].value, 1.5);
        assert_eq!(p.providers[0].provider_name, "test_provider");

        let non_existent = family.get_property("nonexistent");
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_freeze_and_hash() {
        let mut family = MoleculeFamily::new("Freeze Family".into(), None);
        assert!(!family.frozen);
        family.recompute_hash();
        let pre_hash = family.family_hash.clone();
        family.freeze();
        assert!(family.frozen);
        assert!(family.frozen_at.is_some());
        assert!(family.family_hash.is_some());
        assert_ne!(pre_hash, family.family_hash);
    }
}
