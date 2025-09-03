# Sistema ChemFlow

## 1. Objetivo General

ChemFlow orquesta workflows científicos (química / bioinformática) con foco en:

- Reproducibilidad fuerte (hashes canónicos)
- Inmutabilidad de artefactos
- Ejecución tipada de Steps
- Uso simultáneo de múltiples proveedores
- Ramificación auditable (branching)
- Interacción humana controlada
- Event sourcing integral

## 2. Alcance (MVP)

Incluye:

- Registro de moléculas y familias inmutables
- Artefactos derivados (agregados, reportes, tablas intermedias)
- Steps secuenciales con trazabilidad
- Múltiples proveedores por propiedad
- Resolución de valor preferido
- Branching desde cualquier Step Completed
- Registro de eventos y ejecuciones
- Ejecución en DAG paralela
- Scheduler distribuido / HPC
- Reintentos automáticos avanzados

## 3. Modelo Conceptual

### 3.1 DataArtifact (Concepto Unificador)

Artefactos consumidos/producidos por Steps:

- MoleculeFamily
- MolecularPropertyValue (colección por molécula)
- FamilyAggregate / AggregateSet
- ExternalInputArtifact
- ParameterSet
- DerivedTable
- RankedCandidates / SelectionList
- Report / ExportArtifact
- DecisionArtifact

Atributos:

- id (UUID)
- artifact_type (enum)
- hash
- payload (estructura tipada o JSON/BLOB)
- metadata (provider_id, version, schema_version, determinism_flag)
- hash_canónico
- created_by_step_execution_id

Reglas:

- Inmutable
- Referenciable
- Composable (mezcla de tipos de entrada)

### 3.2 Molecule

- inchikey (PK lógico)
- smiles (canónico)
- inchi
- metadata normalizada
  Regla: reuso por inchikey; no duplicación.

### 3.3 MoleculeFamily

- id
- ordered_inchikeys
- build_parameters (normalizados)
- provenance (step / provider / external_input)
- hash_canónico = H(ordered_inchikeys + normalized_parameters + schema_version)
- frozen = true tras creación

### 3.4 Propiedad Molecular

- property_name
- múltiples valores (uno por provider o más)
- campos: value, units?, provider_id@version, step_execution_id, quality_metrics
- preferred_flag (una selección activa por política)
- historia completa retenida

### 3.5 FamilyAggregate

- family_id
- aggregate_name
- value (tipado / JSON)
- provider / method
- step_execution_id

## 4. Providers

Tipos:

- MoleculeProvider
- PropertiesProvider
- AnalysisProvider
- TransformationProvider
- ExternalInputProvider (conceptual envoltorio)
  Atributos mínimos:
- id, version
- capabilities (propiedades / transformaciones / agregados)
- validate(params)
- determinism_flag (deterministic | stochastic)
- environment_meta (opcional)

## 5. Steps (Abstractos)

Características:

- Entradas: 0..N DataArtifacts
- Salidas: 0..N DataArtifacts
- Siempre registran StepExecution + eventos

Tabla categorías (MVP):

| Step Type              | Entradas                 | Salidas                              | Ejemplos                 |
| ---------------------- | ------------------------ | ------------------------------------ | ------------------------ |
| MoleculeAcquisition    | (0) / ExternalInput      | MoleculeFamily                       | ZINC fetch               |
| ExternalInputIngestion | (0)                      | ExternalInputArtifact / ParameterSet | Parámetros iniciales     |
| PropertiesCalculation  | MoleculeFamily           | PropertyValues                       | logP, pKa                |
| DataAggregation        | MoleculeFamily (+ props) | FamilyAggregate                      | mean_logP                |
| Transformation         | MoleculeFamily           | MoleculeFamily (1..N)                | split por MW             |
| Selection / Filtering  | MoleculeFamily           | MoleculeFamily / RankedCandidates    | top N                    |
| HumanApproval          | Arbitrario               | DecisionArtifact / ParameterSet      | aprobación parámetros    |
| Analysis               | MoleculeFamily (+ props) | Aggregate / DerivedTable             | clustering               |
| Reporting / Export     | Mixto                    | Report / ExportArtifact              | reporte JSON/PDF         |
| ParameterSynthesis     | ParameterSets            | ParameterSet                         | derivar config compuesta |

Reglas:

1. Steps sin familias son válidos.
2. Validación previa no produce efectos.
3. Hash de ejecución no depende de fuentes externas mutables posteriores.

## 6. Flow y Ejecución

- Flow = enum tipado de Steps
- Cursor secuencial (MVP)
- parent_flow_uuid (para rama)
- branch_from_step_index
- events reconstruibles

Estados de Step:
Pending | Running | AwaitingUserInput | Completed | Failed(reason) | Skipped(reason) | Cancelled

## 7. Event Sourcing

Eventos mínimos:

- StepStarted
- StepCompleted
- StepFailed {reason}
- StepSkipped {reason}
- UserInteractionRequested {schema}
- UserInteractionProvided {decision_hash}
- ArtifactCreated {artifact_id, type, hash}
- ProviderInvoked {provider_id, version, params_hash}
- PropertyPreferenceAssigned {molecule_id, property_name, provider_id}
- BranchCreated {parent_flow, from_step}

Secuencia: flow_local_sequence monotónica.

## 8. Reproducibilidad

Hash StepExecution = H(
sorted(input_artifact_hashes) +
normalized_parameters +
provider_matrix(id@version; sorted) +
step_type +
engine_version +
schema_version +
determinism_flag +
explicit_seed?
)

Sin entradas: solo parámetros + versiones.
Estocástico: registrar seed; si no => no cache determinista.

## 9. Resolución de Valor Preferido

Flujo:

1. Insertar todos los valores
2. Ejecutar política (priority list / mejor score / menor error / más reciente)
3. Marcar preferred
4. Evento PropertyPreferenceAssigned

## 10. Branching

Pasos:

1. Seleccionar step N (Completed)
2. Capturar outputs (ids + hashes)
3. Crear Flow hijo (parent_flow_uuid, branch_from_step_index=N)
4. Reutilizar artefactos (sin copiar)
5. Reconfigurar steps siguientes

Comparaciones:

- Diferencias de propiedades preferidas
- Agregados divergentes
- Diferencia estructural de familias

## 11. Interacción Humana

HumanApprovalStep:

- AwaitingUserInput bloquea avance
- payload_schema (JSON Schema)
- decision_artifact (hash reproducible)
- audit_fields (futuro: user_id, source)
  Eventos: UserInteractionRequested / UserInteractionProvided

## 12. Persistencia (Esquema Conceptual)

Tablas:

- molecules
- molecule_families
- molecule_family_members
- molecular_properties
- property_preference
- family_aggregates
- data_artifacts
- providers
- step_executions
- step_execution_inputs
- step_execution_outputs
- flow_instances
- flow_events

Índices:

- molecular_properties(molecule_id, property_name)
- property_preference(molecule_id, property_name)
- data_artifacts(hash)
- step_executions(hash)
- flow_events(flow_id, sequence)

## 13. Inmutabilidad

1. Artefactos no cambian
2. Nuevas propiedades => nuevas filas
3. Completed no retrocede
4. Eventos no editables
5. Hash mismatch => inconsistencia (auditoría futura)

## 14. Validación

- Sintáctica
- Semántica
- Dominio
- Política (futuro)

## 15. Observabilidad

- Métricas de rendimiento (tiempos de ejecución, uso de recursos)
- Trazas de ejecución (qué pasos se ejecutaron, en qué orden)
- Registros de eventos (interacciones del usuario, cambios de estado)
- Monitoreo de artefactos (estado, versiones, dependencias)

ChemFlow abstrae operaciones científicas en Steps reproducibles sobre artefactos inmutables. La generalización a DataArtifacts permite flujos flexibles (incluyendo aquellos sin datos moleculares iniciales). El diseño habilita trazabilidad total, branching eficiente, selección transparente de valores y expansión futura a paralelismo y cache determinista.

Fin del documento.
