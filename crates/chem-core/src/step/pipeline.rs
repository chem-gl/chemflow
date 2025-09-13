use std::marker::PhantomData;

use super::{StepDefinition, TypedStep};
use crate::repo::{build_flow_definition_auto, FlowDefinition};

/// Marker trait usado para forzar igualdad de tipos en tiempo de compilación.
///
/// Se implementa únicamente para el mismo tipo: `impl<T> SameAs<T> for T {}`.
pub trait SameAs<T> {}
impl<T> SameAs<T> for T {}

/// Builder tipado para construir pipelines donde la salida de un step
/// coincide con la entrada del siguiente.
///
/// El builder almacena pasos como `Box<dyn StepDefinition>` internamente pero
/// fuerza en tiempo de compilación que los tipos adyacentes sean compatibles
/// mediante `SameAs` y los bounds de `then`.
pub struct Pipe<S: TypedStep + StepDefinition + 'static> {
    steps: Vec<Box<dyn StepDefinition>>,
    _out: PhantomData<<S as TypedStep>::Output>,
}

impl<S: TypedStep + StepDefinition + 'static> Pipe<S> {
    /// Crea una canalización con el primer paso.
    pub fn new(step: S) -> Self {
        Self { steps: vec![Box::new(step)], _out: PhantomData }
    }

    /// Añade un paso verificando `N::Input == S::Output` en tiempo de compilación.
    pub fn then<N>(mut self, next: N) -> Pipe<N>
    where
        N: TypedStep + StepDefinition + 'static,
        <N as TypedStep>::Input: SameAs<<S as TypedStep>::Output>,
    {
        self.steps.push(Box::new(next));
        Pipe::<N> { steps: self.steps, _out: PhantomData }
    }

    /// Genera una `FlowDefinition` a partir de la lista de pasos.
    pub fn build(self) -> FlowDefinition {
        build_flow_definition_auto(self.steps)
    }
}
