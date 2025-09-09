use chem_core::model::ExecutionContext;
use chem_core::CompositeInjector;
use chem_core::ParamInjector;
use serde_json::json;

struct InjA;
impl ParamInjector for InjA {
    fn inject(&self, _base: &serde_json::Value, _ctx: &ExecutionContext) -> serde_json::Value {
        json!({"a": 1, "shared": "fromA"})
    }
}
struct InjB;
impl ParamInjector for InjB {
    fn inject(&self, base: &serde_json::Value, _ctx: &ExecutionContext) -> serde_json::Value {
        // produce an override for key 'shared'
        let out = base.clone();
        if let serde_json::Value::Object(mut m) = out {
            m.insert("b".to_string(), json!(2));
            m.insert("shared".to_string(), json!("fromB"));
            serde_json::Value::Object(m)
        } else {
            json!({"b":2, "shared":"fromB"})
        }
    }
}

#[test]
fn composite_injector_merge_is_deterministic() {
    let base = json!({"base": true, "shared": "base"});
    let ctx = ExecutionContext { input: None,
                                 params: json!({}) };
    let c = CompositeInjector::with_injectors(vec![Box::new(InjA), Box::new(InjB)]);
    let merged = c.apply(&base, &ctx);
    // Expect merge order base -> InjA -> InjB and shallow merge semantics
    assert_eq!(merged.get("base").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(merged.get("a").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(merged.get("b").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(merged.get("shared").and_then(|v| v.as_str()), Some("fromB"));
}
