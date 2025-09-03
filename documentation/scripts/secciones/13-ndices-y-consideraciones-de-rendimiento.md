# Sección 13 - Índices y Consideraciones de Rendimiento

### 13.1 Objetivos

- Latencia p95 < 50ms para lecturas críticas (status de flujo, propiedades preferidas).
- Escalabilidad lineal en EVENT_LOG y MOLECULAR_PROPERTIES sin degradación >20% al duplicar volumen.
- Minimizar writes amplificados (índices estrictamente necesarios).
- Habilitar replay rápido (< N log(steps)) apoyado en índices secuenciales.

### 13.2 Principios de Indexación

| Principio                        | Aplicación                                                               |
| -------------------------------- | ------------------------------------------------------------------------ |
| Índice sólo si query recurrente  | Se evita “por si acaso” (auditar con pg_stat_statements)                 |
| Clave estrecha primero           | Columnas de alta selectividad al inicio (ej: molecule_inchikey)          |
| Parciales para filtros fijos     | preferred=true, status='Pending'                                         |
| Covering (INCLUDE)               | Evitar lookups adicionales cuando se lee sólo un subconjunto             |
| JSONB: GIN sólo si necesario     | Campos JSONB grandes (parameters, aggregation) accedidos por clave → GIN |
| Secuencia → BTREE / BRIN híbrido | EVENT_LOG.seq (BTREE), ts BRIN si partición temporal futura              |
| Evitar duplicidad semántica      | No indexar hash y PK si PK ya contiene hash (salvo consultas frecuentes) |

### 13.3 Índices Recomendados (SQL Propuesto)

```sql
-- MOLECULES
CREATE UNIQUE INDEX IF NOT EXISTS uq_molecules_inchikey ON MOLECULES(inchikey);

-- MOLECULE_FAMILIES
CREATE UNIQUE INDEX IF NOT EXISTS uq_families_family_hash ON MOLECULE_FAMILIES(family_hash);
CREATE INDEX IF NOT EXISTS ix_families_frozen ON MOLECULE_FAMILIES(frozen);

-- MOLECULE_FAMILY_MEMBERS (acceso ordenado)
CREATE UNIQUE INDEX IF NOT EXISTS uq_family_members_order ON MOLECULE_FAMILY_MEMBERS(family_id, position);
CREATE INDEX IF NOT EXISTS ix_family_members_molecule ON MOLECULE_FAMILY_MEMBERS(molecule_inchikey);

-- MOLECULAR_PROPERTIES (búsqueda por molécula + propiedad + preferido)
CREATE INDEX IF NOT EXISTS ix_properties_lookup
  ON MOLECULAR_PROPERTIES(molecule_inchikey, property_name, preferred);
-- Partial: garante unicidad del preferido
CREATE UNIQUE INDEX IF NOT EXISTS uq_properties_preferred
  ON MOLECULAR_PROPERTIES(molecule_inchikey, property_name)
  WHERE preferred = true;
-- Hash de valor (deduplicación y aceleración ingest)
CREATE UNIQUE INDEX IF NOT EXISTS uq_properties_value_hash ON MOLECULAR_PROPERTIES(value_hash);
-- Acceso por propiedad global (agregaciones)
CREATE INDEX IF NOT EXISTS ix_properties_property_name ON MOLECULAR_PROPERTIES(property_name);

-- PROPERTY_PROVENANCE (auditoría y filtrado provider/model)
CREATE INDEX IF NOT EXISTS ix_prov_property_id ON PROPERTY_PROVENANCE(molecular_property_id);
CREATE INDEX IF NOT EXISTS ix_prov_step_id ON PROPERTY_PROVENANCE(step_id);
CREATE INDEX IF NOT EXISTS ix_prov_provider_version ON PROPERTY_PROVENANCE(provider_name, provider_version);

-- FAMILY_AGGREGATES y FAMILY_AGGREGATE_NUMERIC
CREATE UNIQUE INDEX IF NOT EXISTS uq_family_aggregate_hash ON FAMILY_AGGREGATES(aggregate_hash);
CREATE INDEX IF NOT EXISTS ix_family_aggregate_name ON FAMILY_AGGREGATES(family_id, aggregate_name);
CREATE UNIQUE INDEX IF NOT EXISTS uq_family_agg_numeric_hash ON FAMILY_AGGREGATE_NUMERIC(value_hash);
CREATE INDEX IF NOT EXISTS ix_family_agg_numeric_name ON FAMILY_AGGREGATE_NUMERIC(family_id, aggregate_name);

-- WORKFLOW_STEP_EXECUTIONS (estado y replay)
CREATE INDEX IF NOT EXISTS ix_steps_flow_cursor ON WORKFLOW_STEP_EXECUTIONS(root_execution_id, step_id);
CREATE INDEX IF NOT EXISTS ix_steps_branch ON WORKFLOW_STEP_EXECUTIONS(branch_id, step_id);
CREATE INDEX IF NOT EXISTS ix_steps_status ON WORKFLOW_STEP_EXECUTIONS(status);
-- Pending crítico (scheduler)
CREATE INDEX IF NOT EXISTS ix_steps_pending_priority
  ON WORKFLOW_STEP_EXECUTIONS(status, start_time NULLS LAST)
  WHERE status = 'Pending';
-- Parámetros canónicos (cache hit)
CREATE INDEX IF NOT EXISTS ix_steps_parameter_hash ON WORKFLOW_STEP_EXECUTIONS(parameter_hash);
-- Fingerprint opcional (si se materializa)
-- CREATE INDEX ix_steps_fingerprint ON WORKFLOW_STEP_EXECUTIONS(fingerprint);

-- WORKFLOW_STEP_ARTIFACTS (resolución rápida por hash para cache)
CREATE UNIQUE INDEX IF NOT EXISTS uq_artifacts_hash ON WORKFLOW_STEP_ARTIFACTS(artifact_hash);
CREATE INDEX IF NOT EXISTS ix_artifacts_type ON WORKFLOW_STEP_ARTIFACTS(artifact_type);

-- WORKFLOW_STEP_ARTIFACT_LINK (consumo)
CREATE INDEX IF NOT EXISTS ix_artifact_link_artifact ON WORKFLOW_STEP_ARTIFACT_LINK(artifact_id);

-- EVENT_LOG (lectura secuencial y filtrada)
CREATE INDEX IF NOT EXISTS ix_events_flow_seq ON EVENT_LOG(flow_id, seq);
CREATE INDEX IF NOT EXISTS ix_events_step ON EVENT_LOG(step_id);
-- Para grandes volúmenes temporales
-- CREATE INDEX ix_events_ts_brin ON EVENT_LOG USING BRIN(ts);

-- WORKFLOW_BRANCHES
CREATE INDEX IF NOT EXISTS ix_branches_root ON WORKFLOW_BRANCHES(root_flow_id);
CREATE INDEX IF NOT EXISTS ix_branches_parent ON WORKFLOW_BRANCHES(parent_flow_id);

-- STEP_EXECUTION_ERRORS
CREATE INDEX IF NOT EXISTS ix_errors_step_attempt ON STEP_EXECUTION_ERRORS(step_id, attempt_number);
CREATE INDEX IF NOT EXISTS ix_errors_class ON STEP_EXECUTION_ERRORS(error_class);

-- JSONB GIN selectivo (sólo si existe patrón de consulta por clave dinámica)
-- CREATE INDEX ix_steps_parameters_gin ON WORKFLOW_STEP_EXECUTIONS USING GIN (parameters jsonb_path_ops);
```

### 13.4 Patrones de Acceso y Justificación

| Caso de Uso                         | Query típica (simplificada)                                           | Índices apalancados                                   |
| ----------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------- |
| Seleccionar propiedad preferida     | WHERE molecule_inchikey=? AND property_name=? AND preferred=true      | ix_properties_lookup + uq_properties_preferred        |
| Determinar siguiente step pendiente | WHERE status='Pending' ORDER sBY start_time NULLS LAST LIMIT 1        | ix_steps_pending_priority                             |
| Replay flujo                        | SELECT \* FROM EVENT_LOG WHERE flow_id=? ORDER BY seq                 | ix_events_flow_seq                                    |
| Detección de duplicados propiedad   | INSERT ... ON CONFLICT (value_hash)                                   | uq_properties_value_hash                              |
| Cache artifact por hash             | SELECT artifact_id FROM WORKFLOW_STEP_ARTIFACTS WHERE artifact_hash=? | uq_artifacts_hash                                     |
| Ramificación / auditoría árbol      | SELECT \* FROM WORKFLOW_BRANCHES WHERE root_flow_id=?                 | ix_branches_root                                      |
| Resolución de errores recientes     | SELECT \* FROM STEP_EXECUTION_ERRORS WHERE step_id=? ORDER BY attempt | ix_errors_step_attempt                                |
| Agregados numéricos por familia     | WHERE family_id=? AND aggregate_name=?                                | ix_family_agg_numeric_name / ix_family_aggregate_name |
| Proveedor / versión comparativos    | WHERE provider_name=? AND provider_version=?                          | ix_prov_provider_version                              |

### 13.5 Índices Parciales y Beneficios

| Índice Parcial             | Condición             | Beneficio                         |
| -------------------------- | --------------------- | --------------------------------- |
| uq_properties_preferred    | preferred = true      | Evita full-scan en verificación   |
| ix_steps_pending_priority  | status = 'Pending'    | Scheduler O(log N_pending)        |
| (futuro) ix_events_ts_brin | (ninguna, BRIN en ts) | Bajo costo para rangos temporales |

### 13.6 GIN / JSONB Criterios

Agregar GIN sólo si se observan estas firmas en pg_stat_statements (ejemplos):

```text
WHERE parameters ? 'selection_policy'
WHERE parameters @> '{"weights":{"quality":0.5}}'
```

De lo contrario, costo de mantenimiento > beneficio.

### 13.7 Mantenimiento y Observabilidad

| Aspecto           | Config / Acción                                                         |
| ----------------- | ----------------------------------------------------------------------- |
| Monitoreo         | pg_stat_statements, auto_explain (threshold)                            |
| Fragmentación     | REINDEX CONCURRENTLY en índices muy actualizados (raro aquí)            |
| Autovacuum        | Ajustar scale_factor bajo en EVENT_LOG                                  |
| Bloat             | pgstattuple para validar en tablas grandes                              |
| Alertas           | Métricas: idx_scan vs seq_scan (ratio esperado > 0.8 en casos críticos) |
| Planes regresivos | pg_store_plans (opcional)                                               |

### 13.8 Estrategia de Evolución

| Escenario                          | Acción                                                                    |
| ---------------------------------- | ------------------------------------------------------------------------- |
| EVENT_LOG > 200M filas             | Particionar por rango mensual + BRIN en ts                                |
| MOLECULAR_PROPERTIES crecimiento > | Bloom filter index (evaluar) sobre value_hash                             |
| Alta cardinalidad branch_id        | Índice compuesto (branch_id, status) (si consultas frecuentes por estado) |
| JSONB consultas frecuentes         | Activar GIN jsonb_path_ops selectivo                                      |
| Agregados masivos heterogéneos     | Crear FAMILY_AGGREGATE_DISTRIBUTION + índices específicos                 |

### 13.9 Anti‑Patrones Evitados

| Anti‑Patrón                            | Riesgo                      | Mitigación                               |
| -------------------------------------- | --------------------------- | ---------------------------------------- |
| Índices sobre columnas poco selectivas | Inflado de writes           | No indexar flags salvo partial           |
| GIN indiscriminado                     | Alto costo de mantenimiento | Criterios estrictos sección 13.6         |
| Duplicar hash + PK sin uso             | Espacio desperdiciado       | Sólo uniq en hash cuando lookup directo  |
| Falta de índice secuencial             | Replay lento                | ix_events_flow_seq obligatorio           |
| Índice multi-col mal ordenado          | No uso por planner          | Ordenar por cardinalidad / filtro real   |
| JSONB sin índice adecuado              | Costos ocultos              | Auditar consultas y añadir GIN selectivo |

### 13.10 Checklist de Revisión Periódica (Mensual)

1. Top 20 queries (pg_stat_statements) → validar usan índice esperado.
2. Ver ratio idx_scan/seq_scan por tabla.
3. Evaluar crecimiento y necesidad de particionado (EVENT_LOG, STEP_EXECUTION_ERRORS).
4. Confirmar ausencia de índices nunca usados (pg_stat_user_indexes.idx_scan=0).
5. Revisar bloat >30% (pgstattuple) → planear REINDEX.
6. Validar que nuevas políticas (branching / retry) no introdujeron patrones no indexados.

### 13.11 Sistema Base

- PostgreSQL 15.x
- Extensiones sugeridas: pg_stat_statements, auto_explain, (opcional) pg_cron para mantenimiento.
- Futuros: TimescaleDB si métricas/eventos de alta cadencia evolucionan a series temporales.

### 13.12 Métricas Clave a Exportar

| Métrica                           | Fuente                    | Uso                          |
| --------------------------------- | ------------------------- | ---------------------------- |
| chemflow_db_events_lag_ms         | NOW() - MAX(EVENT_LOG.ts) | Detección de retraso ingest  |
| chemflow_db_pending_steps_total   | COUNT(status='Pending')   | Backlog scheduler            |
| chemflow_db_retry_rate_ratio      | retries / completed       | Salud de proveedores / steps |
| chemflow_db_index_cache_hit_ratio | pg_statio_user_indexes    | Ajuste shared_buffers        |
| chemflow_db_seq_scan_ratio        | seq_scan / total_scan     | Eficacia indexación          |
| chemflow_db_bloat_ratio           | pgstattuple               | Monitoreo de bloat           |

### 13.13 Resumen Ejecutivo

Índices priorizados para: (a) lookup determinista (hashes), (b) scheduling (pending), (c) replay (flow_id, seq), (d) selección preferida (partial), (e) auditoría (provider/version). JSONB indexado sólo bajo evidencia empírica. Evolución planificada con partición temporal y BRIN si escala masivo.

---

