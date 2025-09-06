use serde::{de::DeserializeOwned, Serialize};

use super::{StepKind, StepRunResult, StepSignal};
use crate::errors::CoreEngineError;
use crate::model::ArtifactSpec;

/// Resultado de ejecución para Steps tipados.
/// Similar a `StepRunResult` pero expresado en tipos fuertes para outputs.
pub enum StepRunResultTyped<Out: ArtifactSpec + Clone> {
    Success { outputs: Vec<Out> },
    SuccessWithSignals { outputs: Vec<Out>, signals: Vec<StepSignal> },
    Failure { error: CoreEngineError },
}

impl<Out: ArtifactSpec + Clone> StepRunResultTyped<Out> {
    /// Convierte a `StepRunResult` neutro serializando los outputs a
    /// `Artifact`.
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

/// Interfaz de alto nivel para definir Steps fuertemente tipados en
/// Params/Input/Output.
///
/// Ventajas:
/// - Sin acceso por strings a parámetros (serde → `Params`).
/// - Sin JSON dinámico para IO; se usan `ArtifactSpec` de entrada/salida
///   (`Input`/`Output`).
/// - El motor se encarga del puente a la interfaz neutra `StepDefinition`.
pub trait TypedStep {
    type Params: DeserializeOwned + Serialize + Clone + Default;
    type Input: ArtifactSpec + Clone;
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

    /// Ejecución tipada. Para Source, `input` llegará como `None`.
    fn run_typed(&self, input: Option<Self::Input>, params: Self::Params) -> StepRunResultTyped<Self::Output>;
}

// -------------------------------------------------------------
// Adaptador: cualquier `TypedStep` implementa `StepDefinition` neutro.
// -------------------------------------------------------------
impl<T> crate::step::StepDefinition for T where T: TypedStep + 'static
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
        // 1) Decodificar parámetros a `Self::Params`.
        let params: <Self as TypedStep>::Params = ctx.params_as().unwrap_or_else(|_| self.params_default());

        // 2) Decodificar input si existe (para Source vendrá `None`).
        let typed_in: Option<<Self as TypedStep>::Input> = match ctx.input.as_ref() {
            Some(a) => Some(<Self as TypedStep>::Input::from_artifact(a).expect("input artifact decode")),
            None => None,
        };

        // 3) Ejecutar lógica tipada y convertir a resultado neutro.
        <Self as TypedStep>::run_typed(self, typed_in, params).into_neutral()
    }
    fn kind(&self) -> StepKind {
        <Self as TypedStep>::kind(self)
    }
}
