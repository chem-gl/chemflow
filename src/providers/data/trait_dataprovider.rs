//! Trait para proveedores de datos agregados / analíticos.
//! Un `DataProvider` no necesariamente añade propiedades a las familias, sino
//! que produce un resultado JSON (estadísticas, agregaciones, métricas) basado
//! en una o múltiples `MoleculeFamily` de entrada. Su resultado puede ser
//! almacenado como parte de `StepOutput.results` conservando parámetros usados
//! para reproducibilidad.
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::data::family::MoleculeFamily;

#[async_trait]
pub trait DataProvider: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_version(&self) -> &str;
    fn get_description(&self) -> &str;
    fn get_available_parameters(&self) -> HashMap<String, DataParameterDefinition>;

    /// Calcula datos (agregados) sobre una o varias familias. Devuelve un `Value` JSON para
    /// permitir esquemas flexibles. El JSON debería contener suficiente estructura para
    /// mantener trazabilidad (ej. listas de IDs, versión de algoritmo, etc.).
    async fn calculate(
        &self,
        families: &[MoleculeFamily],
        parameters: &HashMap<String, Value>
    ) -> Result<Value, Box<dyn std::error::Error>>;
}

#[derive(Debug, Clone)]
pub struct DataParameterDefinition {
    pub name: String,
    pub description: String,
    pub data_type: DataParameterType,
    pub required: bool,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum DataParameterType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AvgCountProvider;

    #[async_trait]
    impl DataProvider for AvgCountProvider {
        fn get_name(&self) -> &str { "avg_count" }
        fn get_version(&self) -> &str { "0.1.0" }
        fn get_description(&self) -> &str { "Counts total molecules across families" }
        fn get_available_parameters(&self) -> HashMap<String, DataParameterDefinition> { HashMap::new() }
        async fn calculate(&self, families: &[MoleculeFamily], _p: &HashMap<String, Value>) -> Result<Value, Box<dyn std::error::Error>> {
            let total: usize = families.iter().map(|f| f.molecules.len()).sum();
            Ok(Value::Number(serde_json::Number::from(total)))
        }
    }

    #[tokio::test]
    async fn test_dataprovider() {
        let prov = AvgCountProvider;
        assert_eq!(prov.get_name(), "avg_count");
    let _ = prov.get_version();
    let _ = prov.get_description();
        let res = prov.calculate(&[], &HashMap::new()).await.unwrap();
        assert_eq!(res, Value::Number(0.into()));

        let defs = prov.get_available_parameters();
        assert!(defs.is_empty());
    let def = DataParameterDefinition {
            name: "threshold".into(),
            description: "A threshold".into(),
            data_type: DataParameterType::Number,
            required: false,
            default_value: Some(Value::Number(10.into())),
    };
    let _ = &def.name;
    let _ = &def.description;
    match def.data_type { DataParameterType::Number => {}, _ => {} }
    let _ = def.required;
    let _ = def.default_value.clone();
        let _ = DataParameterType::String;
        let _ = DataParameterType::Boolean;
        let _ = DataParameterType::Array;
        let _ = DataParameterType::Object;
    }
}
