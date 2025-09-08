### F6 – Políticas de Selección Básica (chem-policies)

| Núcleo                                                                                      | Contrato Estabilizado           | GATE_F6                                            | Paralelo Seguro                      |
| ------------------------------------------------------------------------------------------- | ------------------------------- | -------------------------------------------------- | ------------------------------------ |
| Trait PropertySelectionPolicy + MaxScore, evento PropertyPreferenceAssigned, preferred flag | Firma choose() + payload evento | Política no altera fingerprint salvo cambio params | Weighted / Consensus (feature gated) |

Objetivos Clave:

- Resolución multi-proveedor determinista.
- Evento auditable con rationale.
- Rationale JSON canónico y conversion a datos tipados.
- verificacion de datos fuertemnete tipados. y funciones parametizables para asegurar que la selección es estable y reproducible.
  Pasos sugeridos:

1. Struct `PropertyCandidate`.
2. Implementación MaxScore (tie-break estable).
3. Emitir evento antes de StepCompleted.
4. Tests: determinismo selección.
5. Parámetros incluidos en fingerprint.
6. Rationale JSON canónico y conversion a datos tipados.
7. Feature flags para políticas extra.
8. disminuir uso de json y usar datos tipados.
   GATE_F6:

- Selección estable en entradas iguales.
- Fingerprint sólo cambia con params/política.

---

## Diagramas

### Diagrama de Clases (F6)

```mermaid
classDiagram
	class PropertySelectionPolicy {
		<<trait>>
		+id() &str
		+choose(cands: &[PropertyCandidate], params: &SelectionParams) SelectionDecision
	}
	class MaxScorePolicy {
		+new()
	}
	class PropertyCandidate {
		+molecule_inchikey: String
		+property_kind: String
		+value: Json
		+units?: String
		+provider?: String
		+version?: String
		+quality?: String
		+score?: f64
		+stable_key() String
		+value_hash() String
	}
	class SelectionParams {
		<<enum>>
		+MaxScore(MaxScoreParams)
	}
	class MaxScoreParams { +tie_break: TieRule }
	class TieRule { <<enum>> +ByKeyThenValueHash }
	class SelectionDecision {
		+selected_key: String
		+policy_id: String
		+params_hash: String
		+rationale: Rationale
	}
	class Rationale {
		+policy_id: String
		+params: SelectionParams
		+considered_n: usize
		+selected_key: String
		+ties: Vec<String>
		+tie_break_rule: TieRule
	}
	class StepDefinition {
		<<trait>>
		+run(ctx) StepRunResult
	}
	class PolicyDemoStep {
		+id()
		+kind()
		+run_typed(..) -> SuccessWithSignals
	}
	class FlowEngine {
		+next_with(..)
		+persist_events(..)
		+compute_flow_fingerprint(..)
	}
	class FlowEventKind {
		<<enum>>
		+PropertyPreferenceAssigned
		+StepStarted
		+StepFinished
		+StepFailed
		+StepSignal
		+FlowInitialized
		+FlowCompleted
	}
	PropertySelectionPolicy <|.. MaxScorePolicy
	SelectionDecision --> Rationale : contiene
	SelectionParams --> MaxScoreParams : usa
	StepDefinition <|.. PolicyDemoStep
	PolicyDemoStep ..> FlowEngine : señales reservadas
	FlowEngine ..> FlowEventKind : emite
	PropertyCandidate ..> SelectionDecision : insumo
```

### Diagrama de Flujo (F6)

```mermaid
flowchart LR
	A[Source/F6Seed] -- Artifact DummyIn --> B[PolicyDemoStep Transform]
	B -- Señal reservada PROPERTY_PREFERENCE_ASSIGNED --> E[(FlowEngine)]
	E -- Traducir --> P{{PropertyPreferenceAssigned}}
	P --> L[event_log]
	E -- StepFinished (fingerprint mezcla params_hash si hubo P) --> L
	E -- FlowCompleted (cuando corresponde) --> L
```

Notas:

- El engine traduce la señal reservada en un evento tipado P antes de `StepFinished` y elimina la señal genérica de la secuencia.
- El fingerprint del step mezcla `params_hash` únicamente cuando existe el evento de política; el flow fingerprint se mantiene determinista.
