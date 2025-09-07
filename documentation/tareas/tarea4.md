### F4 – Adaptadores y Steps Iniciales (chem-adapters)

| Núcleo                                                                              | Contrato Estabilizado                              | GATE_F4                          | Paralelo Seguro                |
| ----------------------------------------------------------------------------------- | -------------------------------------------------- | -------------------------------- | ------------------------------ |
| DomainStepAdapter, AcquireMoleculesStep, ComputePropertiesStep stub, Artifact shape | `Artifact {id, kind, hash, payload, metadata_min}` | Hash artifact estable (snapshot) | Bosquejo Normalize / Aggregate |

Objetivos Clave:

- Traducir dominio a artifacts neutrales.
- Validar pipeline Acquire→Compute.

Plan ordenado (pasos y entregables):

1. Pre-chequeos (sin cambios de código)

- Validar en `chem-core` que `Artifact` tiene forma `{ id, kind, hash, payload, metadata_min }` y que el hash depende sólo del `payload` canónico.
- Verificar que `chem-core` no referencia tipos del dominio (grep `chem_domain`).

1. Contrato de adaptación en `chem-adapters`

- En `crates/chem-adapters/src/lib.rs` definir el trait `DomainArtifactEncoder` para empaquetar dominio → `Artifact` neutral:
  - `encode_molecule(&Molecule) -> Artifact`
  - `encode_family(&MoleculeFamily) -> Artifact`adapter
  - `encode_property(&MolecularProperty) -> Artifact`
    - Despues el artifact donde se combinan estos artifact ya que el paso solo acepta un artifact
- ArtifactKind: en F4 NO se agregan nuevos kinds; se usa `GenericJson` del core y se distingue por el shape del payload y `schema_version`.
- Especificar `payload` canónico mínimo:
  - Molecule: `{ inchikey, smiles?, inchi? }`
  - Family: `{ family_hash, ordered_keys: [inchikey...] }`
  - Property: `{ molecule_inchikey, property_kind, value, units?, provider?, version?, step_id_ref?, family_hash_ref }`
- Entregable: módulo compila + tests unitarios de serialización y hash estable.

1. `AcquireMoleculesStep` (Source determinista)

- Dataset sintético fijo (`synthetic_v1`) con moléculas ordenadas determinísticamente.
- Produce un artifact agregado de familia (payload incluye `family_hash` y `ordered_keys`).
- Validar: el artifact incluye el `family_hash` del dominio y su hash de artifact es estable/reproducible.
- Entregables: step implementado + tests de determinismo y forma de payload.

1. `ComputePropertiesStep` (Transform stub, sin selección)

- Input: FamilyArtifact → Output: un único `FamilyPropertiesArtifact` (agregado) con N items (uno por molécula).
- Valores stub deterministas (p.ej., `score = len(inchikey)`, `units = "au"`).
- Prohibido filtrar: N in == N out; cada artifact referencia `molecule_inchikey` y `family_hash_ref`.
- Entregables: step + tests de no filtrado, referencialidad y hashes estables.

1. Test de integración Acquire→Compute

- Construir `FlowEngine` in-memory con ambos steps.
- Correr 3 veces y afirmar: variantes de eventos idénticas, `flow_fingerprint` idéntico, hash de artifact estable, y N properties == N moléculas.
- Entregable: test verde documentando IDs de steps y dataset `synthetic_v1`.

1. Ejemplo `examples/basic_workflow.toml`

- Contenido mínimo:
  - `[flow] name = "basic_acquire_compute"`
  - `[[steps]] id = "acquire_molecules" kind = "Source" params.dataset = "synthetic_v1"`
  - `[[steps]] id = "compute_properties" kind = "Transform" params.kind = "stub_v1"`
- Añadir snippet de uso en README correspondiente.

1. Verificaciones finales (puertas)

- No filtrado: assert explícito (N in == N out).
- Artifact shape congelado: snapshot JSON de `payload` y `kind`.
- Hash estable: asserts para familia y al menos una property.
- Revisión cero tipos dominio en `chem-core`: repetir grep y anotar OK.

1. Higiene y docs

- Documentar contrato del encoder y `ArtifactKind` en `chem-adapters` con `///` e invariantes.
- Actualizar `documentation/diagramas-final.md` (sección F4) con estado y ejemplo mínimo.
- Opcional: proponer desglose por PRs (Encoder, Acquire, Compute, Integración+Ejemplo).

GATE_F4:

- Pipeline lineal produce artifacts hashables reproducibles.
- Artifact shape congelado.
- Se empieza a escametizar la base de datos.

---

Verificación de implementación (estado actual)

- Encoder dominio→artifact: SimpleDomainEncoder implementado en `crates/chem-adapters/src/encoder.rs`.
- Artifacts tipados: `MoleculeArtifact`, `FamilyArtifact`, `FamilyPropertiesArtifact`, `MolecularPropertyArtifact` en `src/artifacts.rs`.
- Steps:
  - AcquireMoleculesStep (Source determinista) en `src/steps/acquire.rs`.
  - ComputePropertiesStep (Transform stub) en `src/steps/compute.rs`.
- Test de integración: `crates/chem-adapters/tests/integration_f4.rs` valida determinismo (fingerprint/variantes) y N out == 3.
- Ejemplo TOML: `examples/basic_workflow.toml`.
- Ejemplo en main: bloque de demo F4 en `src/main.rs` (in-memory).

Observaciones respecto al plan:

- El kind de artifact permanece `GenericJson` (no se extiende enum en core en F4; distinción por shape y schema_version).
- Hash lo calcula el engine desde payload canónico; los artifacts tipados incluyen `schema_version = 1`.

Diagramas

Diagrama de Flujo (Acquire→Compute)

```mermaid
flowchart LR
	A[AcquireMoleculesStep (Source)] -- FamilyArtifact --> B[ComputePropertiesStep (Transform)]
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
