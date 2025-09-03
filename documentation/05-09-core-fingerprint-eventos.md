## 5. Motor Genérico (Core) – Clases

```mermaid
classDiagram
class FlowEngine { +next(flow_id) +branch(from_step) +resume_user_input(..) +recover(flow_id) }
class FlowDefinition { +steps: Vec<StepDefinition> }
class FlowInstance { +id +cursor +branch_id +status_summary()}
class StepSlot { +defn +status +outputs[] +fingerprint }
class StepDefinition { <<interface>> +id() +name() +kind() +required_input_kinds() +base_params() +validate_params() +run() +fingerprint() +rehydrate() +clone_for_branch() }
class ExecutionContext { +inputs[] +params +event_sink +policy_view }
class Artifact { +id +kind +hash +payload +metadata }
class ParamInjector { <<interface>> +inject(flow,i):Json }
class CompositeInjector { +delegates[] +inject() }
class EventStore { <<interface>> +append() +list(flow_id) }
class FlowRepository { <<interface>> +load() +persist() }
class PolicyEngine { +decide_branch() +decide_retry() }
```

### 5.1 Branching + Retry Diagrama

```mermaid
classDiagram
	class FlowEngine {+next() +branch() +recover() +resume_user_input()}
	class FlowDefinition {+steps: Vec~StepDefinition~}
	class FlowInstance {+id +cursor +branch_id +status_summary()}
	class WORKFLOW_BRANCHES {+branch_id +root_flow_id +parent_flow_id? +created_from_step_id}
	class BranchMetadata {+divergence_params +reason}
	class StepSlot {+index +status +fingerprint +retry_count}
	class StepDefinition
	class RetryPolicy {+should_retry(error,attempt)->bool}
	class ParamInjector
	class CompositeInjector {+delegates[]}
	class ExecutionContext {+inputs[] +params +event_sink}
	class EventStore {+append() +list()}
	class STEP_EXECUTION_ERRORS {+error_id +step_id +attempt_number +error_class}
	class Artifact {+id +kind +hash}
	class PolicyEngine {+decide_branch() +decide_retry()}
	class PropertySelectionPolicy
```

## 6. Ciclo de Vida Step (0–7) + State Machine

Enumeración pasos 0–7 y diagrama de estados:

```mermaid
stateDiagram-v2
	[*] --> Pending
	Pending --> Running: StepStarted
	Running --> Completed: StepCompleted
	Running --> Failed: StepFailed
	Running --> AwaitingUserInput: UserInteractionRequested
	AwaitingUserInput --> Running: UserInteractionProvided
	Pending --> Skipped: StepSkipped
	Failed --> Pending: RetryScheduled
	Pending --> Cancelled: StepCancelled
	Completed --> [*]
	Skipped --> [*]
	Cancelled --> [*]
```

### 6.1 Flujo End-to-End

```mermaid
flowchart LR
    A[Acquire Molecules] --> B[Build Families] --> C[Compute Properties] --> D[Select Preferred Policy] --> E[Aggregate Metrics] --> F{Branch Criteria Met?}
    F -- Yes --> BR[Create Branch] --> C
    F -- No --> G{Human Gate?}
    G -- Yes --> UI[Await User Input] --> G
    G -- No --> H[Generate Report] --> I[Persist Artifacts / Publish]
```

## 7. Inyección de Parámetros y Fusión Determinista

Orden:

```
base_params → injectors → user_overrides → human_gate_payload → (runtime_derived fuera fingerprint)
```

Reglas merge: última clave gana; arrays reemplazan salvo estrategia explícita append.

## 8. Eventos Tipados (Event Sourcing)

| Evento                     | Razón               | Clave                            | Productor |
| -------------------------- | ------------------- | -------------------------------- | --------- |
| FlowCreated                | Nueva instancia     | def_hash                         | Engine    |
| StepStarted                | Cambio estado       | step_id,index                    | Engine    |
| StepValidationFailed       | Rechazo             | error                            | Engine    |
| ProviderInvoked            | Observabilidad      | provider_id,version,params_hash  | Step      |
| ArtifactCreated            | Salida              | artifact_id,kind,hash            | Step      |
| StepCompleted              | Exito               | fingerprint                      | Engine    |
| StepFailed                 | Error runtime       | error_class                      | Engine    |
| StepSkipped                | Política            | reason                           | Engine    |
| UserInteractionRequested   | Gate                | schema,correlation_id            | Engine    |
| UserInteractionProvided    | Gate resuelto       | decision_hash                    | Engine    |
| BranchCreated              | Fork                | parent_flow,from_step,child_flow | Engine    |
| RecoveryStarted            | Inicio recovery     | flow_id                          | Engine    |
| RecoveryCompleted          | Fin recovery        | actions                          | Engine    |
| RetryScheduled             | Retry               | retry_count                      | Engine    |
| PropertyPreferenceAssigned | Selección preferida | molecule,property_id             | Dominio   |

## 9. Fingerprint / Reproducibilidad

Composición mínima:  
sorted(input_hashes) + canonical_json(params_sin_runtime) + step_kind + internal_version + provider_matrix_sorted + schema_version + deterministic_flag (+seed).

Usos: caching, comparación ramas, auditoría divergencias, invalidación selectiva.
