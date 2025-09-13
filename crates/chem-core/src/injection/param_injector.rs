//! Contracto para inyectores de parámetros.
//!
//! Un `ParamInjector` recibe los `base` params del step y el `ExecutionContext`
//! y devuelve un `Value` que será mergeado sobre los params actuales. Los
//! inyectores deben ser deterministas y no provocar efectos secundarios.

use crate::model::ExecutionContext;
use serde_json::Value;

/// Trait para inyectores de parámetros.
pub trait ParamInjector: Send + Sync + std::fmt::Debug {
    /// Devuelve una estructura JSON que será mergeada sobre `base`.
    ///
    /// Implementaciones deben ser deterministas y rápidas.
    fn inject(&self, base: &Value, ctx: &ExecutionContext) -> Value;
}
