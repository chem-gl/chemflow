### F7 – Retry Manual Mínimo

| Núcleo                                                                                    | Contrato Estabilizado          | GATE_F7                                 | Paralelo Seguro                |
| ----------------------------------------------------------------------------------------- | ------------------------------ | --------------------------------------- | ------------------------------ |
| RetryPolicy (should_retry), transición Failed→Pending, retry_count, evento RetryScheduled | Semántica retry (no altera fp) | Reintento no cambia fingerprint ni flow | Backoff diseño inicial (draft) |

Objetivos Clave:

Pasos sugeridos:

1. Campo `retry_count` en StepSlot.
2. `FlowEngine::retry(step_id)`.
3. Evento RetryScheduled (si < max).tarea7
4. Test: exceder max rechaza.
5. Nuevos artifacts generan nuevos IDs (no colisión hash).
6. Métrica interna retries.
7. Documentar semántica.
8. Verificar funciones no verbosas para el fingerprint.
9. usar parametrización para definir políticas de reintento y asegurar que estas políticas no afectan el fingerprint a menos que los parámetros cambien.

GATE_F7:

## Diagramas

### Diagrama de Clases (F7)

```mermaid
classDiagram
	class FlowEngine {
		+schedule_retry(flow_id, def, step_id, reason, max)
		+retries_scheduled()
		+retries_rejected()
		+events_for(flow_id)
	}
	class RetryPolicy {
		+max_retries: u32
		+backoff: BackoffKind
		+should_retry(retry_count)
	}
	class BackoffKind {
		<<enum>>
		+None
		+Exponential(base_ms)
	}
	class StepSlot {
		+retry_count: u32
		+status: StepStatus
	}
	class FlowEventKind {
		<<enum>>
		+RetryScheduled
		+StepFailed
		+StepStarted
		+StepFinished
		+FlowCompleted
	}
	class FlowRepository {
		+load(flow_id, events, def) FlowInstance
	}
	class PgEventStore {
		+append_kind(flow_id, kind)
		+list(flow_id)
	}
	class PgFlowRepository {
		+load(flow_id, events, def)
	}
	FlowEngine --> RetryPolicy : usa
	FlowEngine --> FlowRepository : rehidrata
	FlowEngine --> PgEventStore : persiste eventos
	FlowEngine --> StepSlot : actualiza estado
	StepSlot --> StepStatus
	FlowEngine ..> FlowEventKind : emite
	RetryPolicy --> BackoffKind : compone
	PgEventStore --> FlowEventKind : serializa
	PgFlowRepository --> StepSlot : rehidrata
```

### Diagrama de Flujo (F7)

```mermaid
flowchart LR
	A[StepFailed] -- schedule_retry --> B[FlowEngine]
	B -- RetryPolicy.should_retry --> C{retry_count < max}
	C -- No --> D[rechazado]
	C -- Sí --> E[Emite RetryScheduled]
	E --> F[StepSlot.retry_count++]
	F --> G[StepSlot.status = Pending]
	G --> H[StepStarted]
	H --> I[StepFinished/StepFailed]
	I -- éxito --> J[FlowCompleted]
	I -- fallo --> A
```

Notas:

- El engine sólo permite RetryScheduled si el step está Failed y no excede max_retries.
- El retry no cambia el fingerprint del step ni del flow.
- Artifacts sólo se generan en StepFinished exitoso; no hay duplicados en retries.
