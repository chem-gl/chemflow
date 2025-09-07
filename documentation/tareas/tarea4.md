### F4 – Adaptadores y Steps Iniciales (chem-adapters)

| Núcleo                                                                              | Contrato Estabilizado                              | GATE_F4                          | Paralelo Seguro                |
| ----------------------------------------------------------------------------------- | -------------------------------------------------- | -------------------------------- | ------------------------------ |
| DomainStepAdapter, AcquireMoleculesStep, ComputePropertiesStep stub, Artifact shape | `Artifact {id, kind, hash, payload, metadata_min}` | Hash artifact estable (snapshot) | Bosquejo Normalize / Aggregate |

Objetivos Clave:

- Traducir dominio a artifacts neutrales.
- Validar pipeline Acquire→Compute.

Plan ordenado (pasos y entregables):



Diagramas

Diagrama de Flujo (Acquire→Compute)

```mermaid
flowchart LR
	A[AcquireMoleculesStep Source] -- FamilyArtifact --> B[ComputePropertiesStep Transform]
	B -- FamilyPropertiesArtifact --> C[(FlowEngine)]
	C -- fingerprint/eventos --> D[Verificación]
```

Diagrama de Clases Simplificado (F4)

```mermaid
classDiagram
	class FlowEngine {
		+run_to_end()
		+event_variants()
		+flow_fingerprint()
	}
	class StepDefinition {
		<<trait>>
		+id() String
		+run(ctx) StepRunResult
	}
	class AcquireMoleculesStep {
		+run(..) -> FamilyArtifact
	}
	class ComputePropertiesStep {
		+run(FamilyArtifact) -> FamilyPropertiesArtifact
	}
	class DomainArtifactEncoder {
		<<trait>>
		+encode_molecule(Molecule) Artifact
		+encode_family(MoleculeFamily) Artifact
		+encode_property(MolecularProperty) Artifact
	}
	class SimpleDomainEncoder {
	}
	class Molecule
	class MoleculeFamily
	class MolecularProperty
	class Artifact {
		+kind: GenericJson
		+hash: String
		+payload: Json
	}
	class FamilyArtifact
	class FamilyPropertiesArtifact

	StepDefinition <|.. AcquireMoleculesStep
	StepDefinition <|.. ComputePropertiesStep
	DomainArtifactEncoder <|.. SimpleDomainEncoder
	Molecule "*" --> MoleculeFamily : members
	AcquireMoleculesStep --> FamilyArtifact
	ComputePropertiesStep --> FamilyPropertiesArtifact
	SimpleDomainEncoder --> Artifact
```

Notas

- Este diagrama omite detalles del repo/event store; se centra en F4.
- Los macros `typed_artifact!` y `typed_step!` generan tipos y glue para integrarse con `FlowEngine`.

---
