//! Builder pattern for FlowEngine

use crate::engine::FlowEngine;
use crate::event::EventStore;
use crate::repo::FlowRepository;
use crate::step::{SameAs, StepDefinition, TypedStep};
use std::marker::PhantomData;

/// Builder inicial para configurar un FlowEngine
///
/// Requiere definir el primer paso del flujo antes de poder continuar
#[derive(Debug)]
pub struct EngineBuilderInit<E: EventStore, R: FlowRepository> {
    pub event_store: E,
    pub repository: R,
}

impl<E: EventStore, R: FlowRepository> EngineBuilderInit<E, R> {
    /// Define el primer paso del flujo (debe ser de tipo Source)
    ///
    /// # Ejemplo
    /// ```
    /// let builder = FlowEngine::builder(event_store, repo)
    ///     .first_step(MySourceStep::new());
    /// ```
    #[inline]
    pub fn first_step<S>(self, step: S) -> EngineBuilder<S, E, R>
        where S: TypedStep + std::fmt::Debug + 'static
    {
        debug_assert!(matches!(step.kind(), crate::step::StepKind::Source),
                      "El primer paso debe ser de tipo Source");

        EngineBuilder { event_store: self.event_store,
                        repository: self.repository,
                        steps: vec![Box::new(step)],
                        _out: PhantomData::<S::Output> }
    }
}

/// Builder para agregar pasos adicionales al flujo
///
/// Garantiza en tiempo de compilación que los tipos de entrada y salida
/// de los pasos sean compatibles
#[derive(Debug)]
pub struct EngineBuilder<S: TypedStep + std::fmt::Debug + 'static, E: EventStore, R: FlowRepository> {
    event_store: E,
    repository: R,
    steps: Vec<Box<dyn StepDefinition>>,
    _out: PhantomData<S::Output>,
}

impl<S: TypedStep + std::fmt::Debug + 'static, E: EventStore, R: FlowRepository> EngineBuilder<S, E, R> {
    /// Agrega un nuevo paso al flujo
    ///
    /// # Ejemplo
    /// ```
    /// let builder = builder
    ///     .add_step(MyTransformStep::new())
    ///     .add_step(MyOutputStep::new());
    /// ```
    #[inline]
    pub fn add_step<N>(mut self, next: N) -> EngineBuilder<N, E, R>
        where N: TypedStep + std::fmt::Debug + 'static,
              N::Input: SameAs<S::Output>
    {
        self.steps.push(Box::new(next));
        EngineBuilder { event_store: self.event_store,
                        repository: self.repository,
                        steps: self.steps,
                        _out: PhantomData }
    }

    /// Construye el FlowEngine final con la configuración definida
    ///
    /// # Ejemplo
    /// ```
    /// let engine = builder.build();
    /// ```
    #[inline]
    pub fn build(self) -> FlowEngine<E, R> {
        let mut engine = FlowEngine::new_with_stores(self.event_store, self.repository);
        let definition = crate::repo::build_flow_definition_auto(self.steps);
        engine.set_default_definition(definition);
        engine
    }
}
