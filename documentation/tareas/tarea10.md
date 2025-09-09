### F10 – Inyección Compuesta + Human Gate

| Núcleo                                                                                                      | Contrato Estabilizado                                | GATE_F10                           | Paralelo Seguro      |
| ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------------------------- | -------------------- |
| ParamInjector trait, CompositeInjector, estado AwaitingUserInput, eventos UserInteractionRequested/Provided | Orden merge estable (base→injectors→overrides→human) | Nuevos inyectores por ArtifactKind | UI mínima aprobación |

Objetivos Clave:

Pasos sugeridos:

1. Definir orden merge fijo.
2. CompositeInjector determinista + test.
3. Extender state machine (AwaitingUserInput).
4. `resume_user_input(...)`.
5. Validar schema input humano.
6. decision_hash (rationale).
7. Test fingerprint estable con/ sin gate (mismos overrides).

GATE_F10:

## Diagrama de flujo (flowchart)

El siguiente diagrama muestra el flujo de ejecución usado en el demo añadido a `src/main.rs` (dos corridas: A sin gate y B con gate). Describe cómo el engine aplica inyectores, emite la petición humana y cómo se reanuda.

```mermaid
flowchart TB
	Start([Start])
	BuildDef[/Build FlowDefinition/]
	RegisterInjectors[/Register injectors: FamilyHash & Properties/]
	RunSrc[/Execute Source step - src/]
	CheckHuman{Needs human input?}
	EmitReq["Emit UserInteractionRequested event"]
	AppendEvent[/Append event to InMemoryEventStore/]
	Resume["Call resume_user_input"]
	RunT["Continue execution for step 't' - Transform"]
	ComputeFP[/Compute last_step_fingerprint/]
	Compare{fp_a == fp_b?}
	EndOK([End - fingerprints equal])
	EndDiff([End - fingerprints differ])

	Start --> BuildDef --> RegisterInjectors --> RunSrc --> CheckHuman
	CheckHuman -- No --> RunT --> ComputeFP
	CheckHuman -- Yes --> EmitReq --> AppendEvent --> Resume --> RunT --> ComputeFP
	ComputeFP --> Compare
	Compare -- Yes --> EndOK
	Compare -- No --> EndDiff
```

## Diagrama de clases (classDiagram)

Resumen de las piezas principales implicadas en la tarea y su relación con el demo:

```mermaid
classDiagram
	class ParamInjector {
		<<trait>>
		+inject(params: Value, ctx: ExecutionContext) Value
	}

	class CompositeInjector {
		-injectors: Vec<Box<dyn ParamInjector>>
		+inject_all(base: Value, ctx: ExecutionContext) Value
	}

	class FlowEngine {
		-event_store: Box<dyn EventStore>
		-repo: Box<dyn FlowRepository>
		-injectors: Vec<Box<dyn ParamInjector>>
		+next_with(flow_id: Uuid, def: &FlowDefinition) -> Result
		+resume_user_input(flow_id: Uuid, def: &FlowDefinition, step_id: &str, provided: Value) -> Result
		+last_step_fingerprint(flow_id: Uuid, step_id: &str) -> Result<String>
	}

	class InMemoryEventStore {
		+append_kind(flow_id: Uuid, kind: FlowEventKind)
		+list(flow_id: Uuid) -> Vec<FlowEvent>
	}

	class StepDefinition {
		<<trait>>
		+run(ctx: &ExecutionContext) -> StepRunResult
	}

	class FamilyHashInjector
	class PropertiesInjector

	ParamInjector <|.. CompositeInjector
	CompositeInjector o-- ParamInjector
	FlowEngine o-- InMemoryEventStore
	FlowEngine o-- CompositeInjector
	FlowEngine ..> StepDefinition
	FamilyHashInjector ..|> ParamInjector
	PropertiesInjector ..|> ParamInjector
```

## Notas

- El diagrama de flujo muestra la ruta alternativa cuando se requiere intervención humana: la engine emite un evento `UserInteractionRequested` que puede ser inyectado en el `EventStore` (como en el ejemplo donde se usa `InMemoryEventStore::append_kind`) y luego reanudado con `resume_user_input`.
- El diagrama de clases resume las entidades nuevas/afectadas: el trait `ParamInjector` y su composición, las implementaciones concretas (`FamilyHashInjector`, `PropertiesInjector`), el `FlowEngine` y el `InMemoryEventStore` con su método `append_kind` (accesible tras traer el trait `EventStore` al scope).

Si quieres, puedo también generar una imagen SVG desde Mermaid y añadirla al repositorio, o convertir estas secciones en una página HTML estática dentro de `documentation/`.
