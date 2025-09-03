# Sección 8 - Eventos Tipados (Event Sourcing)

| Evento                     | Razón               | Payload Clave                              | Productor |
| -------------------------- | ------------------- | ------------------------------------------ | --------- |
| FlowCreated                | Nueva instancia     | flow_id, def_hash                          | Engine    |
| StepStarted                | Cambio estado       | step_id, index                             | Engine    |
| StepValidationFailed       | Rechazo temprano    | step_id, error                             | Engine    |
| ProviderInvoked            | Observabilidad      | step_id, provider_id, version, params_hash | Step      |
| ArtifactCreated            | Registro salida     | step_id, artifact_id, kind, hash           | Step      |
| StepCompleted              | Cierre exitoso      | step_id, fingerprint                       | Engine    |
| StepFailed                 | Error runtime       | step_id, error_class                       | Engine    |
| StepSkipped                | Política skip       | step_id, reason                            | Engine    |
| UserInteractionRequested   | Gate humano         | step_id, schema, correlation_id            | Engine    |
| UserInteractionProvided    | Gate resuelto       | step_id, decision_hash                     | Engine    |
| BranchCreated              | Fork reproducible   | parent_flow, from_step, child_flow         | Engine    |
| RecoveryStarted            | Inicio recovery     | flow_id                                    | Engine    |
| RecoveryCompleted          | Fin recovery        | flow_id, actions                           | Engine    |
| RetryScheduled             | Retry programado    | step_id, retry_count                       | Engine    |
| PropertyPreferenceAssigned | Selección preferida | molecule, property_name, property_id       | Dominio   |

Todos ordenados por `seq` monotónico. No se reescriben ni borran.

