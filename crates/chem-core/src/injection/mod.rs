//! Inyección de parámetros determinista (F10 minimal).
//!
//! Este módulo ofrece un contrato para aplicar inyectores de parámetros
//! sobre los `base` params de un step antes de ejecutarlo. El objetivo es
//! permitir extensiones deterministas (por ejemplo añadir contexto runtime)
//! sin cambiar la semántica del step.
//!
//! Submódulos:
//! - `param_injector`: trait `ParamInjector`.
//! - `composite`: `CompositeInjector` que aplica una lista de inyectores.
//! - `merge`: utilitario `merge_json` para merges JSON.

pub mod param_injector;
pub mod composite;
pub mod merge;

pub use param_injector::ParamInjector;
pub use composite::CompositeInjector;
pub use merge::merge_json;
