/// Estado runtime de un Step durante la ejecución del Flow.
///
/// Transiciones válidas:
/// - Pending -> Running (al emitir StepStarted)
/// - Running -> FinishedOk (al emitir StepFinished)
/// - Running -> Failed (al emitir StepFailed) No se permiten reversiones ni
///   saltos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    /// El engine espera input humano antes de continuar con el step.
    AwaitingUserInput,
    FinishedOk,
    Failed,
}
