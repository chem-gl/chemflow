use serde::{de::DeserializeOwned, Serialize};

use super::{StepKind, StepRunResult, StepSignal};
use crate::errors::CoreEngineError;
use crate::model::ArtifactSpec;

/// Resultado tipado de ejecutar un `TypedStep`.
///
/// Permite trabajar con outputs fuertemente tipados durante la implementación
/// de pasos y convertirlos a la representación neutra que el engine usa.
pub enum StepRunResultTyped<Out: ArtifactSpec + Clone> {
    Success { outputs: Vec<Out> },
    SuccessWithSignals { outputs: Vec<Out>, signals: Vec<StepSignal> },
    Failure { error: CoreEngineError },
}

impl<Out: ArtifactSpec + Clone> StepRunResultTyped<Out> {
    /// Convierte a `StepRunResult` neutro serializando los outputs a
    /// `Artifact` usando `ArtifactSpec::into_artifact`.
    pub fn into_neutral(self) -> StepRunResult {
        match self {
            StepRunResultTyped::Success { outputs } => {
                let arts = outputs.into_iter().map(|o| o.into_artifact()).collect();
                StepRunResult::Success { outputs: arts }
            }
            StepRunResultTyped::SuccessWithSignals { outputs, signals } => {
                let arts = outputs.into_iter().map(|o| o.into_artifact()).collect();
                StepRunResult::SuccessWithSignals { outputs: arts, signals }
            }
            StepRunResultTyped::Failure { error } => StepRunResult::Failure { error },
        }
    }
}

/// Interfaz de alto nivel para definir Steps con tipos fuertes
/// (Params / Input / Output).
///
/// Implementadores escriben `run_typed` con tipos concretos; un adaptador
/// (abajo) convierte esa ejecución a la interfaz neutra `StepDefinition`.
pub trait TypedStep {
    /// Parámetros deserializables y clonables (soportan `Default`).
    type Params: DeserializeOwned + Serialize + Clone + Default;
    /// Tipo concreto esperado como input (implementa `ArtifactSpec`).
    type Input: ArtifactSpec + Clone;
    /// Tipo concreto producido como output (implementa `ArtifactSpec`).
    type Output: ArtifactSpec + Clone;

    /// Identificador estable del step dentro del flow.
    fn id(&self) -> &'static str;

    /// Nombre amigable (por defecto usa el id).
    fn name(&self) -> &str {
        self.id()
    }

    /// Tipo general del step.
    fn kind(&self) -> StepKind;

    /// Parámetros por defecto deterministas.
    fn params_default(&self) -> Self::Params {
        Default::default()
    }

    /// Ejecución tipada. Para `Source`, `input` será `None`.
    fn run_typed(&self, input: Option<Self::Input>, params: Self::Params) -> StepRunResultTyped<Self::Output>;
}

// -------------------------------------------------------------
// Adaptador: cualquier `TypedStep` implementa `StepDefinition` neutro.
// -------------------------------------------------------------
impl<T> crate::step::StepDefinition for T where T: TypedStep + 'static + std::fmt::Debug
{
    fn id(&self) -> &str {
        self.id()
    }

    fn name(&self) -> &str {
        <Self as TypedStep>::name(self)
    }

    fn base_params(&self) -> serde_json::Value {
        serde_json::to_value(self.params_default()).expect("serialize default params")
    }

    fn run(&self, ctx: &crate::model::ExecutionContext) -> StepRunResult {
        // Decodifica los params (si fallan, usa defaults del step)
        let params: <Self as TypedStep>::Params = ctx.params_as().unwrap_or_else(|_| self.params_default());

        // Decodifica input si existe
        let typed_in: Option<<Self as TypedStep>::Input> =
            ctx.input
               .as_ref()
               .map(|a| <Self as TypedStep>::Input::from_artifact(a).expect("input artifact decode"));

        <Self as TypedStep>::run_typed(self, typed_in, params).into_neutral()
    }

    fn kind(&self) -> StepKind {
        <Self as TypedStep>::kind(self)
    }

    fn definition_hash(&self) -> String {
        let hash_input = serde_json::json!({
            "id": self.id(),
            "kind": format!("{:?}", self.kind()),
            "base_params": self.base_params(),
            "type": std::any::type_name::<T>()
        });
        crate::hashing::hash_value(&hash_input)
    }
}
