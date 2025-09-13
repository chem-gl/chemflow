//! Builder para `FlowEngine`.
//!
//! Este módulo implementa un patrón builder seguro en tiempo de compilación
//! que obliga a declarar el primer paso (fuente) y encadenar pasos cuyos
//! tipos de entrada y salida sean compatibles.
//!
//! Notas de diseño
//! - `EngineBuilderInit` representa el estado inicial del builder: stores
//!   (event_store + repository) deben estar presentes.
//! - `EngineBuilder<S, E, R>` mantiene el último tipo de salida conocido
//!   `S::Output` (mediante `PhantomData`) y la lista de pasos en forma de
//!   `Vec<Box<dyn StepDefinition>>`.
//! - El método `add_step` impone en sus bounds que la entrada del siguiente
//!   paso sea compatible con la salida del paso anterior usando `SameAs`.
//!
//! Ejemplo de uso (comentario):
//!
//! ```ignore
//! // Construcción típica:
//! // let engine = FlowEngine::new()
//! //     .first_step(SourceStep)
//! //     .add_step(TransformStep)
//! //     .add_step(SinkStep)
//! //     .build();
//! ```

use std::fmt::Debug;
use std::marker::PhantomData;

use crate::engine::FlowEngine;
use crate::event::EventStore;
use crate::repo::FlowRepository;
use crate::step::{SameAs, StepDefinition, TypedStep};

/// Estado inicial del builder.
///
/// Contiene las stores necesarias para crear un `FlowEngine`. Antes de poder
/// añadir pasos debemos definir el primer paso (de tipo `Source`).
#[derive(Debug)]
pub struct EngineBuilderInit<E: EventStore, R: FlowRepository> {
    /// Store de eventos que usará el engine.
    pub event_store: E,
    /// Repositorio de definiciones/estado del flujo.
    pub repository: R,
}

impl<E: EventStore, R: FlowRepository> EngineBuilderInit<E, R> {
    /// Define el primer paso del flujo y transiciona al builder completo.
    ///
    /// Requerimos que el primer paso sea de tipo `Source`. Se hace una
    /// aserción en tiempo de ejecución (`debug_assert!`) para ayudar durante
    /// el desarrollo; en builds release la aserción queda desactivada.
    #[inline]
    pub fn first_step<S>(self, step: S) -> EngineBuilder<S, E, R>
        where S: TypedStep + Debug + 'static
    {
        // ...existing code...
        // Ayuda al desarrollador: el primer paso conceptualmente debe ser una fuente
        debug_assert!(matches!(step.kind(), crate::step::StepKind::Source),
                      "El primer paso debe ser de tipo Source",);

        EngineBuilder { event_store: self.event_store,
                        repository: self.repository,
                        steps: vec![Box::new(step)],
                        _out: PhantomData::<S::Output> }
    }
}

/// Builder principal que acumula pasos y garantiza compatibilidad de tipos.
///
/// El parámetro genérico `S` representa el tipo del último `TypedStep`
/// añadido al builder; su asociado `S::Output` se conserva en `_out` para
/// imponer restricciones en el siguiente `add_step`.
#[derive(Debug)]
pub struct EngineBuilder<S: TypedStep + Debug + 'static, E: EventStore, R: FlowRepository> {
    event_store: E,
    repository: R,
    /// Lista de pasos que conforman la definición del flujo.
    steps: Vec<Box<dyn StepDefinition>>,
    /// Marcador de tipo para el output del último paso añadido.
    _out: PhantomData<S::Output>,
}

impl<S: TypedStep + Debug + 'static, E: EventStore, R: FlowRepository> EngineBuilder<S, E, R> {
    /// Añade un siguiente paso al flujo.
    ///
    /// La comprobación `N::Input: SameAs<S::Output>` asegura que la entrada del
    /// nuevo paso `N` es compatible con la salida del paso anterior `S`.
    ///
    /// Consumimos `self` porque cambiamos el estado del builder y devolvemos
    /// un nuevo `EngineBuilder` parametrizado por el nuevo paso `N`.
    #[inline]
    pub fn add_step<N>(mut self, next: N) -> EngineBuilder<N, E, R>
        where N: TypedStep + Debug + 'static,
              N::Input: SameAs<S::Output>
    {
        self.steps.push(Box::new(next));

        EngineBuilder { event_store: self.event_store,
                        repository: self.repository,
                        steps: self.steps,
                        _out: PhantomData }
    }

    // ...existing code...
    /// Construye el `FlowEngine` final usando las stores y la lista de pasos.
    ///
    /// Este método consume el builder. Genera automáticamente la definición
    /// del flujo a partir de `self.steps` (mediante la función del repo) y la
    /// establece como definición por defecto del engine.
    #[inline]
    pub fn build(self) -> FlowEngine<E, R> {
        let mut engine = FlowEngine::new_with_stores(self.event_store, self.repository);
        let definition = crate::repo::build_flow_definition_auto(self.steps);
        engine.set_default_definition(definition);
        engine
    }
}
