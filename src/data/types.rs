//! Tipos de datos moleculares genéricos y estructuras concretas utilizadas
//! para asociar valores cuantitativos con trazabilidad (fuente, timestamp,
//! estado de congelación). El trait `MolecularData` define una interfaz común
//! para diferentes tipos de datos (LogP, toxicidad, etc.).
use serde::{Deserialize, Serialize, de::DeserializeOwned};
pub trait MolecularData: Serialize + DeserializeOwned + Send + Sync {
    /// Tipo nativo subyacente (ej: f64 para datos numéricos continuos).
    type NativeType;
    /// Obtiene el valor principal.
    fn get_value(&self) -> &Self::NativeType;
    /// Fuente / método / proveedor que originó el dato.
    fn get_source(&self) -> &str;
    /// Indica si el dato está "congelado" (inmutable para reproducibilidad /
    /// branching).
    fn is_frozen(&self) -> bool;
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPData {
    pub value: f64,
    pub source: String,
    pub frozen: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
impl MolecularData for LogPData {
    type NativeType = f64;
    fn get_value(&self) -> &Self::NativeType {
        &self.value
    }
    fn get_source(&self) -> &str {
        &self.source
    }
    fn is_frozen(&self) -> bool {
        self.frozen
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    #[test]
    fn test_logp_data_methods() {
        let data = LogPData { value: 2.5,
                              source: "test_source".to_string(),
                              frozen: true,
                              timestamp: Utc::now() };
        assert_eq!(*data.get_value(), 2.5);
        assert_eq!(data.get_source(), "test_source");
        assert!(data.is_frozen());
    }
}
#[cfg(test)]
mod molecular_data_trait_tests {
    use super::*;
    use chrono::Utc;
    #[test]
    fn test_logp_data_trait_methods_direct() {
        let data = LogPData { value: 3.14,
                              source: "src".to_string(),
                              frozen: false,
                              timestamp: Utc::now() };
        // Directly use the trait methods on the concrete type
        assert_eq!(*data.get_value(), 3.14);
        assert_eq!(data.get_source(), "src");
        assert!(!data.is_frozen());
    }
}
