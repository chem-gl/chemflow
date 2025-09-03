# Sección 20 - Auditoría y Trazabilidad Completa

Objetivo: Permitir reconstruir de forma determinista (a) qué ocurrió, (b) en qué orden, (c) con qué parámetros, (d) qué entradas y salidas exactas tuvo cada step, (e) qué decisiones (automáticas o humanas) influyeron, y (f) cómo divergieron (branching) y convergieron los flujos.

### 20.1 Clases Core Involucradas

- FlowEngine: orquestador; emite eventos.
- FlowInstance: identidad del flujo + branch_id.
- StepSlot: estado y fingerprint de cada ejecución de step.
- StepDefinition: lógica determinista (contrato).
- Artifact: salida inmutable referenciada por hash.
- EventStore: fuente de verdad temporal.
- PolicyEngine / PropertySelectionPolicy / RetryPolicy: producen decisiones auditables vía eventos.
- ParamInjector / CompositeInjector: afectan parámetros finales (guardados en WORKFLOW_STEP_EXECUTIONS.parameters).

### 20.2 Tablas Principales y Rol

| Tabla                                    | Rol                                                                                |
| ---------------------------------------- | ---------------------------------------------------------------------------------- |
| EVENT_LOG                                | Secuencia total ordenada (replay, reconstrucción de timeline).                     |
| WORKFLOW_STEP_EXECUTIONS                 | Estado materializado por step: status, parámetros canónicos, branch_id, tiempos.   |
| WORKFLOW_STEP_ARTIFACTS                  | Artifacts producidos (hash, tipo, payload).                                        |
| WORKFLOW_STEP_ARTIFACT_LINK              | Referencias de consumo (qué step usó qué artifact).                                |
| WORKFLOW_BRANCHES                        | Metadatos de ramas (origen, divergencia).                                          |
| STEP_EXECUTION_ERRORS                    | Historial completo de fallos y reintentos.                                         |
| MOLECULAR_PROPERTIES / FAMILY_AGGREGATES | Datos de dominio vinculables a steps vía provenance (PROPERTY_PROVENANCE.step_id). |
| PROPERTY_PROVENANCE                      | Traza desde valores/aggregates hasta el step que los originó.                      |
| WORKFLOW_STEP_RESULTS (opcional)         | Resultados estructurados adicionales (scores, metrics).                            |

### 20.3 Ejes de Trazabilidad

1. Temporal (orden): EVENT_LOG.seq y ts.
2. Lógico (pipeline): índice de step (derivable de StepStarted/StepCompleted) + WORKFLOW_STEP_EXECUTIONS.
3. Paramétrico: WORKFLOW_STEP_EXECUTIONS.parameters + parameter_hash (comparación entre ramas).
4. Artefactual: WORKFLOW_STEP_ARTIFACTS (producción) + WORKFLOW_STEP_ARTIFACT_LINK (consumo) → grafo de dependencia.
5. Dominio: PROPERTY_PROVENANCE enlaza propiedades/aggregates con step_id y provider.
6. Decisiones: eventos BranchCreated, PropertyPreferenceAssigned, UserInteractionRequested/Provided, RetryScheduled registran razones y hashes de divergencia/rationale.
7. Errores: STEP_EXECUTION_ERRORS.attempt_number permite analizar evolución y clasificación (error_class, transient).

### 20.4 Procedimiento de Reconstrucción (Replay Forense)

Paso a paso:

1. Cargar EVENT_LOG filtrando por flow_id (y sus branches: WORKFLOW_BRANCHES.root_flow_id).
2. Reproducir estado: aplicar eventos en orden (state machine) para reconstituir statuses.
3. Para cada StepCompleted:
   - Obtener parámetros canónicos (WORKFLOW_STEP_EXECUTIONS.parameters).
   - Calcular fingerprint esperado y contrastar con el registrado en evento (si se persiste en payload) o reproducido.
4. Resolver entradas: consultar WORKFLOW_STEP_ARTIFACT_LINK por step_id → listar artifacts (hash) y verificar existencia en WORKFLOW_STEP_ARTIFACTS.
5. Resolver salidas: WORKFLOW_STEP_ARTIFACTS.produced_in_step = step_id.
6. Alinear dominio: para cada propiedad agregada al dominio que posea PROPERTY_PROVENANCE.step_id = step_id.
7. Analizar branching: WORKFLOW_BRANCHES (branch_id, created_from_step_id) + evento BranchCreated payload (divergence_params_hash) → comparar parameter_hash antes/después.
8. Auditar selección preferida: evento PropertyPreferenceAssigned + MOLECULAR_PROPERTIES(preferred=true) asegura consistencia.
9. Inspeccionar reintentos: STEP_EXECUTION_ERRORS ordenado por attempt_number hasta estado terminal.
10. Validar integridad hash: recomputar hash(payload) de artifacts y comparar con artifact_hash (unicidad).

### 20.5 Consultas Clave

Último estado sintetizado por step:

```sql
SELECT se.step_id, se.status, se.parameter_hash, se.retry_count
FROM WORKFLOW_STEP_EXECUTIONS se
WHERE se.branch_id = $1
ORDER BY se.start_time;
```

Gráfo de dependencias (producción → consumo):

```sql
SELECT p.artifact_id, p.artifact_type, c.step_id AS consumed_in_step
FROM WORKFLOW_STEP_ARTIFACTS p
LEFT JOIN WORKFLOW_STEP_ARTIFACT_LINK c ON c.artifact_id = p.artifact_id
WHERE p.produced_in_step = $1;
```

Reconstrucción de rationale de selección preferida:

```sql
SELECT e.seq, e.payload
FROM EVENT_LOG e
WHERE e.event_type = 'PropertyPreferenceAssigned'
  AND (e.payload->>'molecule') = $1
  AND (e.payload->>'property_name') = $2
ORDER BY e.seq DESC LIMIT 1;
```

### 20.6 Matriz Clase ↔ Tabla (Resumen)

| Clase / Concepto               | Tabla Primaria                                                                | Enlaces Secundarios                    |
| ------------------------------ | ----------------------------------------------------------------------------- | -------------------------------------- |
| FlowInstance                   | (implícito en EVENT_LOG.flow_id) + WORKFLOW_STEP_EXECUTIONS.root_execution_id | WORKFLOW_BRANCHES                      |
| StepSlot / StepDefinition exec | WORKFLOW_STEP_EXECUTIONS                                                      | EVENT_LOG, STEP_EXECUTION_ERRORS       |
| Artifact                       | WORKFLOW_STEP_ARTIFACTS                                                       | WORKFLOW_STEP_ARTIFACT_LINK            |
| Property (valor)               | MOLECULAR_PROPERTIES                                                          | PROPERTY_PROVENANCE, eventos selección |
| Aggregate                      | FAMILY_AGGREGATES / FAMILY_AGGREGATE_NUMERIC                                  | PROPERTY_PROVENANCE (si aplica)        |
| Branch                         | WORKFLOW_BRANCHES                                                             | EVENT_LOG (BranchCreated)              |
| Retry intentos                 | STEP_EXECUTION_ERRORS                                                         | EVENT_LOG (RetryScheduled)             |
| Selección preferida            | MOLECULAR_PROPERTIES(preferred)                                               | EVENT_LOG (PropertyPreferenceAssigned) |

### 20.7 Garantías Clave

- No ambigüedad temporal: seq monotónico.
- No pérdida de relación entrada→salida: ambos lados referencian artifact_id/hash.
- Divergencia reproducible: parameter_hash + divergence_params_hash permiten comparar ramas.
- Idempotencia forense: re‑ejecutar replay no altera tablas (lectura pura).
- Aislamiento de semántica química: Core sólo necesita artifact_kind y hashes; el resto proviene de tablas de dominio.

### 20.8 Validaciones Recomendadas (Jobs de Consistencia)

1. Artifacts huérfanos (sin consumo ni retención): política de retención.
2. Propiedades preferidas sin evento correspondiente (alerta).
3. Eventos StepCompleted sin WORKFLOW_STEP_EXECUTIONS (inconsistencia transaccional).
4. property_preferred duplicado (asegurado por índice parcial; monitorear violaciones).
5. Divergence_params_hash ausente en una rama ≠ raíz (alerta integridad).

### 20.9 Resumen

La trazabilidad completa se fundamenta en: EVENT_LOG como única fuente temporal + materializaciones normalizadas (WORKFLOW_STEP_EXECUTIONS, WORKFLOW_STEP_ARTIFACTS) + enlaces de consumo + proveniencia de dominio. Cada decisión crítica se refleja como evento tipado firmable y todas las reconstrucciones se derivan de datos inmutables (hash‑backed).

---

