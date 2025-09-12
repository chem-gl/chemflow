//! Inyección de parámetros determinista (F10 minimal).
use crate::model::ExecutionContext;
use serde_json::Value;

/// Trait para inyectores de parámetros.
pub trait ParamInjector: Send + Sync + std::fmt::Debug {
    /// Toma los `base` params del step y el contexto de ejecución y devuelve
    /// una extensión/overrides que será mergeada según el orden fijo.
    fn inject(&self, base: &Value, ctx: &ExecutionContext) -> Value;
}

/// CompositeInjector aplica una serie de inyectores en orden, de forma
/// determinista. El orden de merge es: base -> injectors (en orden) ->
/// overrides -> human
pub struct CompositeInjector {
    pub injectors: Vec<Box<dyn ParamInjector>>,
}

impl CompositeInjector {
    pub fn new() -> Self {
        Self { injectors: vec![] }
    }

    pub fn with_injectors(inj: Vec<Box<dyn ParamInjector>>) -> Self {
        Self { injectors: inj }
    }

    /// Merge determinista: base then each injector result deep-merged (shallow
    /// object merge), returning the merged params. `overrides` and `human`
    /// are applied by callers after this method.
    pub fn apply(&self, base: &Value, ctx: &ExecutionContext) -> Value {
        let mut accumulated = base.clone();
        for i in self.injectors.iter() {
            let v = i.inject(base, ctx);
            accumulated = merge_json(&accumulated, &v);
        }
        accumulated
    }

    /// Helper that applies a slice of injector trait objects by reference
    /// without taking ownership. This is convenient for callers that store
    /// injectors in `FlowEngine` and want to apply them without moving
    /// boxes.
    pub fn apply_injectors(injectors: &[Box<dyn ParamInjector>], base: &Value, ctx: &ExecutionContext) -> Value {
        let mut accumulated = base.clone();
        for inj in injectors.iter() {
            let v = inj.inject(base, ctx);
            accumulated = merge_json(&accumulated, &v);
        }
        accumulated
    }
}

/// Simple shallow merge for JSON objects: keys from `b` override `a`.
fn merge_json(a: &Value, b: &Value) -> Value {
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut out = ma.clone();
            for (k, v) in mb.iter() {
                out.insert(k.clone(), v.clone());
            }
            Value::Object(out)
        }
        // Non-objects: override
        (_, other) => other.clone(),
    }
}
