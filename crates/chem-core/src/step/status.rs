/// Estado runtime de un Step durante la ejecución del Flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus { Pending, Running, FinishedOk, Failed }
