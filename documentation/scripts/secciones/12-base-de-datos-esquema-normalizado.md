# Sección 12 - Base de Datos – Esquema Normalizado

```mermaid
erDiagram
        MOLECULES ||--o{ MOLECULE_FAMILY_MEMBERS : member
        MOLECULE_FAMILIES ||--o{ MOLECULE_FAMILY_MEMBERS : contains
        MOLECULE_FAMILIES ||--o{ FAMILY_AGGREGATES : has
        MOLECULES ||--o{ MOLECULAR_PROPERTIES : has
        MOLECULAR_PROPERTIES ||--o{ PROPERTY_PROVENANCE : provenance
        FAMILY_PROPERTIES ||--o{ PROPERTY_PROVENANCE : provenance_family
        WORKFLOW_BRANCHES ||--o{ WORKFLOW_STEP_EXECUTIONS : groups
        WORKFLOW_STEP_EXECUTIONS ||--o{ WORKFLOW_STEP_FAMILY : links
        WORKFLOW_STEP_EXECUTIONS ||--o{ WORKFLOW_STEP_ARTIFACTS : produces
        WORKFLOW_STEP_EXECUTIONS ||--o{ WORKFLOW_STEP_ARTIFACT_LINK : consumes
        WORKFLOW_STEP_EXECUTIONS ||--o{ WORKFLOW_STEP_RESULTS : produces
        WORKFLOW_STEP_ARTIFACTS ||--o{ WORKFLOW_STEP_ARTIFACT_LINK : referenced
        WORKFLOW_STEP_EXECUTIONS ||--o{ STEP_EXECUTION_ERRORS : logs
        WORKFLOW_STEP_EXECUTIONS ||--o{ EVENT_LOG : emits

        MOLECULES {
            TEXT inchikey PK
            TEXT smiles
            TEXT inchi
            TEXT common_name
        }

        MOLECULE_FAMILIES {
            UUID id PK
            JSONB provenance
            BOOLEAN frozen
            TEXT family_hash
        }

        MOLECULE_FAMILY_MEMBERS {
            UUID family_id FK
            TEXT molecule_inchikey FK
            INT position
        }

        MOLECULAR_PROPERTIES {
            UUID id PK
            TEXT molecule_inchikey FK
            TEXT property_name
            JSONB value
            TEXT units
            BOOLEAN preferred
            TEXT value_hash
        }

        FAMILY_PROPERTIES {
            UUID id PK
            UUID family_id FK
            TEXT property_name
            JSONB aggregation
            TEXT aggregation_method
            TEXT aggregation_hash
        }

        PROPERTY_PROVENANCE {
            UUID provenance_id PK
            UUID molecular_property_id FK
            UUID family_property_id FK
            TEXT provider_name
            TEXT provider_version
            UUID step_id FK
        }

        FAMILY_AGGREGATES {
            UUID id PK
            UUID family_id FK
            TEXT aggregate_name
            JSONB aggregate_value
            TEXT aggregate_hash
            TEXT method
        }

        FAMILY_AGGREGATE_NUMERIC {
            UUID id PK
            UUID family_id FK
            TEXT aggregate_name
            DOUBLE value
            TEXT method
            TEXT value_hash
        }

        WORKFLOW_BRANCHES {
            UUID branch_id PK
            UUID root_flow_id
            UUID parent_flow_id
            UUID created_from_step_id FK
            TIMESTAMPTZ created_at
            TEXT reason
            JSONB divergence_params
        }

        WORKFLOW_STEP_EXECUTIONS {
            UUID step_id PK
            UUID branch_id FK
            TEXT status
            INT retry_count
            INT max_retries
            JSONB parameters
            TEXT parameter_hash
            JSONB providers_used
            TIMESTAMPTZ start_time
            TIMESTAMPTZ end_time
            UUID root_execution_id
            UUID parent_step_id
            UUID branch_from_step_id
        }

        WORKFLOW_STEP_FAMILY {
            UUID step_id FK
            UUID family_id FK
            TEXT role
        }

        WORKFLOW_STEP_ARTIFACTS {
            UUID artifact_id PK
            TEXT artifact_type
            TEXT artifact_hash
            JSONB payload
            JSONB metadata
            UUID produced_in_step FK
        }

        WORKFLOW_STEP_ARTIFACT_LINK {
            UUID step_id FK
            UUID artifact_id FK
            TEXT role
        }

        WORKFLOW_STEP_RESULTS {
            UUID step_id FK
            TEXT result_key
            JSONB result_value
            TEXT result_type
            TEXT result_hash
        }

        STEP_EXECUTION_ERRORS {
            UUID error_id PK
            UUID step_id FK
            INT attempt_number
            TIMESTAMPTZ ts
            TEXT error_class
            TEXT error_code
            TEXT message
            JSONB details
            BOOLEAN transient
        }

        EVENT_LOG {
            BIGSERIAL seq PK
            UUID flow_id
            UUID step_id
            TEXT event_type
            JSONB payload
            TIMESTAMPTZ ts
        }
```

### 12.1 Estandarización de Nombres (Consistencia)

Inconsistencias identificadas y resolución:

| Anterior                              | Nuevo Estándar                                    | Acción                        |
| ------------------------------------- | ------------------------------------------------- | ----------------------------- |
| MOLECULAR_PROPERTY_PROVENANCE         | PROPERTY_PROVENANCE                               | Renombrado (alias de compat.) |
| (Proyección) FamilyPropertyProjection | FAMILY_PROPERTIES                                 | Materialización opcional      |
| FAMILY_AGGREGATES (JSONB genérico)    | FAMILY_AGGREGATE_NUMERIC (y otras especializadas) | Se añaden tablas normalizadas |

`MOLECULAR_PROPERTY_PROVENANCE` se mantiene como alias interno sólo para migraciones; toda documentación nueva usa `PROPERTY_PROVENANCE`.

### 12.2 Normalización de Agregados

Razón: `aggregate_value JSONB` puede introducir heterogeneidad (tipos numéricos vs distribuciones). Estrategia híbrida:

1. Mantener `FAMILY_AGGREGATES` (compatibilidad / agregados complejos no tabulares).
2. Añadir tablas especializadas por tipo dominante:
   - `FAMILY_AGGREGATE_NUMERIC`: valores escalares (media, mediana, desviación, conteos normalizados).
   - Futuro: `FAMILY_AGGREGATE_DISTRIBUTION` (p.ej. histogramas con bins normalizados) si se requiere.

Beneficios: índices específicos, consultas más eficientes, constraints de tipo y menor ambigüedad semántica.

### 12.3 Branching – Tabla `WORKFLOW_BRANCHES`

Campos clave:

| Campo                | Descripción                                     |
| -------------------- | ----------------------------------------------- |
| branch_id            | Identificador único de la rama                  |
| root_flow_id         | Flujo original (rama raíz)                      |
| parent_flow_id       | Flujo padre inmediato (NULL si raíz)            |
| created_from_step_id | Step del padre donde se hizo el fork            |
| divergence_params    | JSON con parámetros cambiados respecto al padre |
| reason               | Texto libre / etiqueta (exploración, fix, tune) |

### 12.4 Retries – Campos en `WORKFLOW_STEP_EXECUTIONS`

`retry_count` aumenta en cada transición Failed→Pending. `max_retries` permite a un PolicyEngine decidir corte; si se excede, estado final = Failed (terminal) y se produce evento `RetryScheduled` sólo mientras `retry_count < max_retries`.

### 12.5 Registro de Errores – `STEP_EXECUTION_ERRORS`

Cada error se inserta con `attempt_number` correlacionado; esto permite reconstruir timeline exacto y analizar patrones de fallos intermitentes.

### 12.6 Consultas Ejemplo

Obtener últimos errores transitorios antes de un retry:

```sql
SELECT * FROM STEP_EXECUTION_ERRORS
WHERE step_id = $1 AND transient = true
ORDER BY attempt_number DESC LIMIT 3;
```

Recuperar todas las ramas divergentes desde un flow raíz:

```sql
SELECT b.branch_id, b.reason, count(se.step_id) AS steps
FROM WORKFLOW_BRANCHES b
LEFT JOIN WORKFLOW_STEP_EXECUTIONS se ON se.branch_id = b.branch_id
WHERE b.root_flow_id = $1
GROUP BY b.branch_id, b.reason;
```

Fingerprint comparativo entre ramas (inputs idénticos, params divergentes):

```sql
SELECT se.branch_id, se.step_id, se.parameter_hash, se.status
FROM WORKFLOW_STEP_EXECUTIONS se
WHERE se.root_execution_id = $1 AND se.status = 'Completed';
```

