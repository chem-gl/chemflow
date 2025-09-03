## 20. Auditoría y Trazabilidad Completa

### 20.1 Clases Core

FlowEngine, FlowInstance, StepSlot, StepDefinition, Artifact, EventStore, PolicyEngine, ParamInjector.

### 20.2 Tablas Principales

EVENT_LOG, WORKFLOW_STEP_EXECUTIONS, WORKFLOW_STEP_ARTIFACTS, WORKFLOW_STEP_ARTIFACT_LINK, WORKFLOW_BRANCHES, STEP_EXECUTION_ERRORS, MOLECULAR_PROPERTIES, PROPERTY_PROVENANCE, WORKFLOW_STEP_RESULTS (opcional).

### 20.3 Ejes Trazabilidad

Temporal, lógico, paramétrico, artefactual, dominio, decisiones, errores.

### 20.4 Procedimiento Replay Forense

Pasos 1–10 (carga eventos → verificación hashes).

### 20.5 Consultas Clave

Incluye SQL (estado steps, dependencias artifacts, rationale selección).

### 20.6 Matriz Clase ↔ Tabla

Tabla reproducida (FlowInstance → WORKFLOW_STEP_EXECUTIONS, etc.)

### 20.7 Garantías

Seq monotónico, relaciones entrada/salida, divergencia reproducible, idempotencia replay, aislamiento semántico.

### 20.8 Validaciones Jobs

Artifacts huérfanos, preferidos sin evento, StepCompleted sin ejecución, duplicados preferidos, ausencia divergence_params_hash.

### 20.9 Resumen

EVENT_LOG + materializaciones normalizadas + proveniencia = trazabilidad completa inmutable.
