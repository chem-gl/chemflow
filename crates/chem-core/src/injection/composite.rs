//! `CompositeInjector`: aplica una secuencia de `ParamInjector` de forma
//! determinista y devuelve los params resultantes.
//!
//! Este módulo contiene una implementación concreta que conserva la interfaz
//! original del proyecto: `CompositeInjector` con un vector de `Box<dyn
//! ParamInjector>`.

use crate::model::ExecutionContext;
use serde_json::Value;

use super::merge::merge_json;
use super::param_injector::ParamInjector;

/// Error simple para la composición (placeholder para futuras políticas).
#[derive(Debug)]
pub enum CompositeError {
    /// Un injector falló al producir un valor (actualmente no ocurre, pero lo
    /// dejamos para futuras extensiones).
    InjectorFailed,
}

/// CompositeInjector aplica una serie de inyectores en orden, de forma
/// determinista. El orden de merge es: base -> injectors (en orden).
#[derive(Debug)]
pub struct CompositeInjector {
    pub injectors: Vec<Box<dyn ParamInjector>>,
}

impl CompositeInjector {
    /// Crea un `CompositeInjector` vacío.
    pub fn new() -> Self {
        Self { injectors: vec![] }
    }

    /// Crea un `CompositeInjector` con la lista dada de inyectores.
    pub fn with_injectors(inj: Vec<Box<dyn ParamInjector>>) -> Self {
        Self { injectors: inj }
    }

    /// Aplica los inyectores sobre `base` y devuelve los params resultantes.
    pub fn apply(&self, base: &Value, ctx: &ExecutionContext) -> Value {
        let mut accumulated = base.clone();
        for i in self.injectors.iter() {
            let v = i.inject(&accumulated, ctx);
            accumulated = merge_json(&accumulated, &v);
        }
        accumulated
    }

    /// Versión estática que aplica un slice de inyectores sin tomar
    /// ownership (útil para callers que mantienen inyectores en `FlowEngine`).
    pub fn apply_injectors(injectors: &[Box<dyn ParamInjector>], base: &Value, ctx: &ExecutionContext) -> Value {
        let mut accumulated = base.clone();
        for inj in injectors.iter() {
            let v = inj.inject(&accumulated, ctx);
            accumulated = merge_json(&accumulated, &v);
        }
        accumulated
    }
}

impl Default for CompositeInjector {
    fn default() -> Self {
        Self::new()
    }
}
