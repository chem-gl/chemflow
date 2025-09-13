/// Estado de un Step en tiempo de ejecución.
///
/// Las transiciones válidas son:
/// - `Pending` -> `Running`
/// - `Running` -> `FinishedOk`
/// - `Running` -> `Failed`
///
/// No se permiten reversiones o saltos arbitrarios entre estados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    /// El paso está pendiente de ejecución.
    Pending,
    /// El paso está en ejecución.
    Running,
    /// El paso espera intervención del usuario.
    AwaitingUserInput,
    /// El paso finalizó correctamente.
    FinishedOk,
    /// El paso falló.
    Failed,
}
