use chem_core::model::ExecutionContext;
use chem_core::ParamInjector;
use serde_json::json;

/// Simple injector that injects the FamilyArtifact's family_hash into params
/// under key `family_hash` when an input artifact is present.
pub struct FamilyHashInjector;

impl ParamInjector for FamilyHashInjector {
    fn inject(&self, _base: &serde_json::Value, ctx: &ExecutionContext) -> serde_json::Value {
        if let Some(input) = &ctx.input {
            let mut out = serde_json::Map::new();
            out.insert("family_hash".to_string(), json!(input.hash));
            serde_json::Value::Object(out)
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        }
    }
}

/// Injector that extracts a `properties` field from the input artifact payload
/// and injects a lightweight `properties_summary` param (e.g. count of items)
/// Useful as an example of an injector that reads artifact payload content.
pub struct PropertiesInjector;

impl ParamInjector for PropertiesInjector {
    fn inject(&self, _base: &serde_json::Value, ctx: &ExecutionContext) -> serde_json::Value {
        if let Some(input) = &ctx.input {
            if let Some(props) = input.payload.get("properties") {
                let count = match props {
                    serde_json::Value::Array(a) => a.len(),
                    serde_json::Value::Object(o) => o.len(),
                    _ => 1,
                };
                let mut out = serde_json::Map::new();
                out.insert("properties_summary".to_string(), json!({"count": count}));
                return serde_json::Value::Object(out);
            }
        }
        serde_json::Value::Object(serde_json::Map::new())
    }
}
