## 3. Modelo de Dominio – Diagramas e Invariantes

### 3.1 Class Diagram (Dominio Puro)

```mermaid
classDiagram
class Molecule { +inchikey: String +smiles: String +inchi: String +metadata: Json }
class MoleculeFamily { +id: UUID +ordered_keys: Vec<inchikey> +family_hash: String +provenance: Json +frozen: bool }
class MolecularProperty { +id: UUID +molecule: inchikey +name: String +value: Json +units?: String +quality?: String +value_hash: String +preferred: bool }
class FamilyAggregate { +id: UUID +family_id: UUID +aggregate_name: String +aggregate_value: Json +aggregate_hash: String +method: String }
class FamilyPropertyProjection { <<computed>> +family_id +property_name +values[] +preferred }

MoleculeFamily --> Molecule : contiene orden
MolecularProperty --> Molecule : describe
FamilyAggregate --> MoleculeFamily : deriva
FamilyPropertyProjection --> MoleculeFamily : agrupa
```

### 3.6 Diagrama de Clases Dominio ↔ Core (Químico → Artefactos)

````mermaid
classDiagram
    class Molecule {+inchikey: String +smiles: String +inchi: String +metadata: Json}
    class MoleculeFamily {+id: UUID +ordered_keys: Vec<inchikey> +family_hash: String +frozen: bool}
    class MolecularProperty {+id: UUID +molecule_inchikey: String +kind: String +value: Json +units?: String +preferred: bool}
    class FamilyAggregate {+id: UUID +family_id: UUID +aggregate_name: String +aggregate_value: Json +aggregate_hash: String}
    class DomainStepAdapter {+collect_inputs(): Vec~Artifact~ +emit_artifacts(): Vec~Artifact~ +validate_domain_objs()}
    class Artifact {+id: UUID +kind: ArtifactKind +hash: String +payload: Json +metadata: Json}
    class ArtifactKind {<<enum>> Molecule, Family, Property, Aggregate}
    class StepDefinition {<<interface>> +id() +name() +kind() +required_input_kinds() +base_params() +validate_params() +run() +fingerprint() +rehydrate() +clone_for_branch()}
    class PropertySelectionPolicy {+select(properties): preferred +rationale: Json}
    class FlowEvent {+seq: u64 +ts: DateTime +event_type: String +payload: Json}
    class FlowEngine {+next(flow_id) +branch(from_step) +recover(flow_id) +resume_user_input(params)}
    class EventStore {+append(event) +list(flow_id) +replay(flow_id)}

    MoleculeFamily --> Molecule : contiene orden
    MolecularProperty --> Molecule : describe
    FamilyAggregate --> MoleculeFamily : deriva
    DomainStepAdapter ..> MolecularProperty : adapta propiedades
    DomainStepAdapter ..> FamilyAggregate : adapta agregados
    DomainStepAdapter --> Artifact : empaqueta en
    Artifact --> ArtifactKind : clasifica como
    StepDefinition --> Artifact : produce/consume
    StepDefinition --> ArtifactKind : declara tipos requeridos
    PropertySelectionPolicy ..> StepDefinition : configura política
    FlowEngine ..> StepDefinition : ejecuta
    FlowEngine ..> FlowEvent : emite
    FlowEngine --> EventStore : persiste eventos
    EventStore --> FlowEvent : almacena
```### 3.2 Invariantes Dominio

| ID   | Invariante             | Descripción                                                                   | Enforcement                              |
| ---- | ---------------------- | ----------------------------------------------------------------------------- | ---------------------------------------- |
| INV1 | inchikey único         | Una molécula por inchikey                                                     | PK MOLECULES                             |
| INV2 | Familia congelada      | No se altera `ordered_keys` tras primer uso como INPUT                        | flag frozen + rechazo mutaciones         |
| INV3 | Hash consistente       | family_hash = hash(ordered_keys normalizado)                                  | Recalcular y comparar antes de persistir |
| INV4 | Propiedad inmutable    | value_hash identifica valor; nunca se edita in situ                           | Insert‑only; cambios = nuevo registro    |
| INV5 | Aggregate determinista | aggregate_hash depende sólo (family_hash, params método)                      | Recomputar y validar colisión            |
| INV6 | preferred único        | A lo sumo un MolecularProperty preferred=(true) por (molecule, property_name) | índice parcial único                     |

### 3.3 Taxonomía de Propiedades Moleculares (Dominio)

Categorías (ejemplos — extensible):

- Fisicoquímicas: LogP, LogD, pKa, LogS (solubilidad), MW, PSA, VolumenMolar, RefraccionMolar.
- Estructurales: RotoresLibres, Polarizabilidad, CargaParcialAtómica.
- Electrónicas: EnergiaHOMO, EnergiaLUMO, EnergiaHidratación.
- Biológicas (predichas): PermeabilidadCaco2, LD50, ToxicidadPredicha.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum MolecularPropertyKind {
    LogP,
    LogD,
    PKa,
    LogS,
    PesoMolecular,
    PSA,
    VolumenMolar,
    RefraccionMolar,
    RotoresLibres,
    Polarizabilidad,
    CargaParcialAtomica,
    EnergiaHOMO,
    EnergiaLUMO,
    EnergiaHidratacion,
    PermeabilidadCaco2,
    LD50,
    ToxicidadPredicha,
}
````

### 3.4 Inmutabilidad y Proveniencia de Propiedades

| Campo                 | Descripción                        | Motivo                 |
| --------------------- | ---------------------------------- | ---------------------- |
| molecule_inchikey     | Identidad molécula                 | Foreign key            |
| property_kind         | Enum estable                       | Consistencia & queries |
| value                 | JSON normalizado                   | Flexibilidad           |
| units                 | Unidades SI/estándar               | Comparabilidad         |
| provider_name/version | Proveniencia exacta                | Reproducibilidad       |
| step_id               | Step que produjo el valor          | Trazabilidad           |
| quality               | Score/confianza                    | Resolución conflictos  |
| preferred             | Selección                          | Fast lookup            |
| value_hash            | Hash(value+units+provider+version) | Inmutabilidad          |

### 3.5 Resolución Multi‑Proveedor

Modelo de política, scoring, evento `PropertyPreferenceAssigned`, rationale JSON y mitigaciones de riesgos (oscilación, empates, unidades, outliers) descritos. Diagrama de secuencia incluido.

```mermaid
sequenceDiagram
    participant Provider1 as Provider 1
    participant Provider2 as Provider 2
    participant ScoringEngine as Scoring Engine
    participant DB as Database
    Provider1->>ScoringEngine: Submit property value (value, units, quality)
    Provider2->>ScoringEngine: Submit property value (value, units, quality)
    ScoringEngine->>ScoringEngine: Score values based on policy (weights, freshness, min_quality)
    ScoringEngine->>ScoringEngine: Select preferred (resolve ties, outliers)
    ScoringEngine->>DB: Emit PropertyPreferenceAssigned event (rationale JSON)
    DB->>DB: Update preferred flag in MOLECULAR_PROPERTIES
```

Parámetros recomendados para determinismo: providers, selection_policy, weights, freshness_half_life_days, min_quality, aggregate_method (opcional).
