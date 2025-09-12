//! Flow context implementation

use crate::engine::FlowEngine;
use crate::errors::CoreEngineError;
use crate::event::EventStore;
use crate::repo::FlowRepository;
use crate::FlowDefinition;
use uuid::Uuid;

/// Contexto de ejecución para un flujo específico
///
/// Proporciona una API ergonómica para ejecutar pasos y gestionar el estado
/// de un flujo dentro de un FlowEngine
pub struct FlowCtx<'a, E: EventStore, R: FlowRepository> {
    pub engine: &'a mut FlowEngine<E, R>,
    pub flow_id: Uuid,
    pub definition: &'a FlowDefinition,
}

impl<'a, E: EventStore, R: FlowRepository> FlowCtx<'a, E, R> {
    /// Crea un nuevo contexto de flujo
    #[inline]
    pub fn new(engine: &'a mut FlowEngine<E, R>, flow_id: Uuid, definition: &'a FlowDefinition) -> Self {
        Self { engine,
               flow_id,
               definition }
    }

    /// Ejecuta el siguiente paso del flujo
    #[inline]
    pub fn step(&mut self) -> Result<(), CoreEngineError> {
        self.engine.next_with(self.flow_id, self.definition)
    }

    /// Ejecuta hasta `n` pasos o hasta que ocurra un error terminal
    #[inline]
    pub fn run_n(&mut self, n: usize) -> Result<(), CoreEngineError> {
        for _ in 0..n {
            match self.step() {
                Ok(()) => continue,
                Err(CoreEngineError::FlowCompleted) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Ejecuta pasos hasta que el flujo complete o ocurra un error terminal
    #[inline]
    pub fn run_to_completion(&mut self) -> Result<(), CoreEngineError> {
        loop {
            match self.step() {
                Ok(()) => continue,
                Err(CoreEngineError::FlowCompleted) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }
}
