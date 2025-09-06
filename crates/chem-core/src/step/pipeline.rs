use std::marker::PhantomData;

use super::{StepDefinition, TypedStep};
use crate::repo::{build_flow_definition_auto, FlowDefinition};

/// Marker trait to assert two types are the same at compile time.
/// Implemented only for identical types (T: SameAs<T> for all T).
pub trait SameAs<T> {}
impl<T> SameAs<T> for T {}

/// Typed pipeline builder that enforces at compile time that the next step's
/// input matches the previous step's output.
///
/// Usage:
///   let pipe = Pipe::new(SeedStep).then(SumStep).then(NextStep);
///   let definition: FlowDefinition = pipe.build();
pub struct Pipe<S: TypedStep + 'static> {
    steps: Vec<Box<dyn StepDefinition>>,
    _out: PhantomData<<S as TypedStep>::Output>,
}

impl<S: TypedStep + 'static> Pipe<S> {
    pub fn new(step: S) -> Self {
        Self { steps: vec![Box::new(step)],
               _out: PhantomData }
    }

    /// Append a new step, enforcing N::Input == S::Output at compile time.
    pub fn then<N>(mut self, next: N) -> Pipe<N>
        where N: TypedStep + 'static,
              <N as TypedStep>::Input: SameAs<<S as TypedStep>::Output>
    {
        self.steps.push(Box::new(next));
        Pipe::<N> { steps: self.steps,
                    _out: PhantomData }
    }

    /// Build a FlowDefinition from the typed pipeline. The compile-time checks
    /// provided by `then` ensure adjacency compatibility prior to boxing.
    pub fn build(self) -> FlowDefinition {
        build_flow_definition_auto(self.steps)
    }
}
