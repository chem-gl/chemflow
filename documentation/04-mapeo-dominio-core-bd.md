## 4. Mapeo Dominio ↔ Core ↔ Base de Datos

### 4.1 Diagrama de Mapeo

```mermaid
flowchart LR
  subgraph D[Domain Inmutable]
    Molecule
    MoleculeFamily
    MolecularProperty
    FamilyAggregate
  end
  subgraph ACL[Adapter / ACL]
    DomainStepAdapter
    ChemArtifactEncoder
  end
  subgraph C[Core Generico]
    StepDefinition
    Artifact
    FlowEngine
    FlowInstance
    EventStore
  end
  subgraph DB[Persistencia DB]
    MOLECULES[(MOLECULES)]
    MOLECULE_FAMILIES[(MOLECULE_FAMILIES)]
    MOLECULE_FAMILY_MEMBERS[(MOLECULE_FAMILY_MEMBERS)]
    MOLECULAR_PROPERTIES[(MOLECULAR_PROPERTIES)]
    PROPERTY_PROVENANCE[(PROPERTY_PROVENANCE)]
    FAMILY_AGGREGATES[(FAMILY_AGGREGATES / NUMERIC)]
    WORKFLOW_STEP_ARTIFACTS[(WORKFLOW_STEP_ARTIFACTS)]
    WORKFLOW_STEP_EXECUTIONS[(WORKFLOW_STEP_EXECUTIONS)]
    EVENT_LOG[(EVENT_LOG)]
    WORKFLOW_BRANCHES[(WORKFLOW_BRANCHES)]
    STEP_EXECUTION_ERRORS[(STEP_EXECUTION_ERRORS)]
  end
  Molecule --> MOLECULES
  MoleculeFamily --> MOLECULE_FAMILIES
  MoleculeFamily --> MOLECULE_FAMILY_MEMBERS
  MolecularProperty --> MOLECULAR_PROPERTIES
  MolecularProperty --> PROPERTY_PROVENANCE
  FamilyAggregate --> FAMILY_AGGREGATES
  MoleculeFamily --> DomainStepAdapter
  MolecularProperty --> DomainStepAdapter
  FamilyAggregate --> DomainStepAdapter
  DomainStepAdapter --> ChemArtifactEncoder
  ChemArtifactEncoder --> Artifact
  StepDefinition --> Artifact
  FlowEngine --> StepDefinition
  FlowEngine --> FlowInstance
  FlowEngine --> EventStore
  EventStore --> EVENT_LOG
  Artifact --> WORKFLOW_STEP_ARTIFACTS
  FlowInstance --> WORKFLOW_STEP_EXECUTIONS
  FlowEngine --> WORKFLOW_BRANCHES
  FlowEngine --> STEP_EXECUTION_ERRORS
  MOLECULE_FAMILIES ---|family_hash| MoleculeFamily
  MOLECULAR_PROPERTIES ---|value_hash| MolecularProperty
  FAMILY_AGGREGATES ---|aggregate_hash| FamilyAggregate
  WORKFLOW_STEP_ARTIFACTS ---|artifact_hash| Artifact
  WORKFLOW_STEP_EXECUTIONS ---|parameter_hash| StepDefinition
  EVENT_LOG ---|flow_id/step_id| WORKFLOW_STEP_EXECUTIONS
  WORKFLOW_BRANCHES ---|branch_id| WORKFLOW_STEP_EXECUTIONS
```

### 4.2 Relación Canónica

| Concepto              | Persistencia                                | Identidad                |
| --------------------- | ------------------------------------------- | ------------------------ |
| Molecule              | MOLECULES.inchikey                          | inchikey                 |
| MoleculeFamily        | MOLECULE_FAMILIES.family_hash               | family_hash              |
| MolecularProperty     | MOLECULAR_PROPERTIES.value_hash             | value_hash               |
| FamilyAggregate       | FAMILY_AGGREGATES.aggregate_hash            | aggregate_hash           |
| Artifact              | WORKFLOW_STEP_ARTIFACTS.artifact_hash       | artifact_hash            |
| Step Execution        | WORKFLOW_STEP_EXECUTIONS.step_id            | step_id                  |
| Parámetros Step       | WORKFLOW_STEP_EXECUTIONS.parameter_hash     | parameter_hash           |
| Evento                | EVENT_LOG.seq                               | seq                      |
| Branch                | WORKFLOW_BRANCHES.branch_id                 | branch_id                |
| Error ejecución       | STEP_EXECUTION_ERRORS(step_id,attempt)      | attempt_number compuesto |
| Preferencia propiedad | MOLECULAR_PROPERTIES.preferred + evento     | preferred=true           |
| Divergencia rama      | WORKFLOW_BRANCHES.divergence_params (+hash) | divergence_params_hash   |

### 4.3 Flujo de Persistencia

1. Congelar dominio (hashes).
2. Adaptar a artifacts neutrales.
3. Ejecutar Step → eventos + artifacts.
4. EVENT_LOG fuente temporal.
5. Branching agrega metadata sin duplicar histórico.
6. Retries agregan intentos inmutables.

### 4.4 Principios de Integridad

- Hash verificado antes de insert.
- Insert-only para datos inmutables.
- Unicidad preferido vía índice parcial.
- Fingerprint recalculable (no autoridad).
- Branch auditable sin copia física.
