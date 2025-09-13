//! Contrato neutral y tipos auxiliares para Steps del motor.
//!
//! Un Step es una unidad determinista que, a partir de un `ExecutionContext`
//! (inputs + params), produce cero o más `Artifact` de salida o un error.
//!
//! Reglas clave:
//! - Debe ser determinista: la salida depende únicamente de `ExecutionContext`.
//! - El primer step de un flujo suele ser `StepKind::Source` (no requiere input).
//! - Los Steps no deben tener efectos secundarios observables por el engine.
//!
//! Este módulo expone:
//! - `StepKind`: clasificación general del step.
//! - `StepDefinition`: interfaz neutra usada por el engine para ejecutar pasos.
//! - Un helper `definition_hash` para generar un fingerprint básico de la
//!   definición del step (id, kind y base_params).

use serde_json::{json, Value};

use super::run_result::StepRunResult;
use crate::model::ExecutionContext;

/// Clasificación general de un Step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Genera artifacts sin entrada (fuente del flujo).
    Source,
    /// Transformación que recibe un artifact y produce outputs.
    Transform,
    /// Paso final / consumidor.
    Sink,
    /// Paso de chequeo/validación.
    Check,
}

/// Interfaz neutra utilizada por el engine para ejecutar un step.
/// La intención es que implementaciones de alto nivel (por ejemplo desde
/// `TypedStep`) adapten sus tipos y devuelvan `StepRunResult`.
pub trait StepDefinition: std::fmt::Debug {
    /// Identificador estable del step dentro de la definición del flujo.
    fn id(&self) -> &str;

    /// Nombre amigable opcional (por defecto es el `id`).
    fn name(&self) -> &str {
        self.id()
    }

    /// Parámetros deterministas por defecto serializados como `serde_json::Value`.
    ///
    /// Estos parámetros pueden ser fusionados con overrides en tiempo de ejecución
    /// por el engine o por inyectores externos.
    fn base_params(&self) -> Value;

    /// Ejecuta el step de forma pura. Debe depender únicamente de `ctx`.
    ///
    /// Para `Source` el `ExecutionContext::input` típicamente será `None`.
    fn run(&self, ctx: &ExecutionContext) -> StepRunResult;

    /// Tipo general del step (Source/Transform/Sink/Check).
    fn kind(&self) -> StepKind;

    /// Hash sencillo de la definición del step para fingerprinting.
    ///
    /// Por simplicidad se crea un JSON con `id`, `kind` y `base_params` y
    /// se hashea con `crate::hashing::hash_value`.
    fn definition_hash(&self) -> String {
        let hash_input = json!({
            "id": self.id(),
            "kind": format!("{:?}", self.kind()),
            "base_params": self.base_params()
        });
        crate::hashing::hash_value(&hash_input)
    }
}

// Permite usar `Box<dyn StepDefinition>` y delegar las llamadas al objeto
// envuelto. Esto facilita construir vectores homogéneos de pasos en runtimes.
impl StepDefinition for Box<dyn StepDefinition> {
    fn id(&self) -> &str {
        (**self).id()
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    fn base_params(&self) -> Value {
        (**self).base_params()
    }

    fn run(&self, ctx: &ExecutionContext) -> StepRunResult {
        (**self).run(ctx)
    }

    fn kind(&self) -> StepKind {
        (**self).kind()
    }

    fn definition_hash(&self) -> String {
        (**self).definition_hash()
    }
}
