# Sección 24 - Orden de Creación de Componentes (Secuencia Recomendada)

Objetivo: consolidar primero los contratos inmutables (tipos de dominio, firmas de traits, forma de eventos y algoritmo de fingerprint) y después capas progresivamente más volátiles (políticas, branching, inyección avanzada, observabilidad, caching). El orden minimiza refactors porque cada fase sólo depende de contratos congelados en fases previas.

### 24.1 Vista Global (Mapa de Dependencias)

```
Fundaciones → Dominio → Motor Lineal → Persistencia (mem → Postgres) → Adaptadores/Steps
→ Políticas Básicas → Retry → Errores Persistidos → Branching → Inyección Avanzada / Human Gate
→ Políticas Avanzadas → Agregados Normalizados → Observabilidad → Hardening / Caching
```

### 24.2 Convenciones

- Núcleo: aquello imprescindible que define el “contrato” de la fase.
- Contrato Estabilizado: partes que se congelan al cerrar la fase (no se tocan en adelante salvo migración controlada).
- GATE_Fx: condición objetiva de cierre de fase.
- Paralelo Seguro: trabajo que puede hacerse en paralelo sin romper el contrato ya fijado.
- Pasos sugeridos: secuencia concreta de implementación dentro de la fase.

---

### F0 – Fundaciones del Workspace

| Núcleo                                                                                    | Contrato Estabilizado                      | GATE_F0                                              | Paralelo Seguro                                |
| ----------------------------------------------------------------------------------------- | ------------------------------------------ | ---------------------------------------------------- | ---------------------------------------------- |
| Matriz de dependencias (check_deps.sh), formato (rustfmt), clippy baseline, pin toolchain | Reglas de capa aceptadas + estilo uniforme | Script check_deps.sh pasa sin violaciones y CI verde | README/CONTRIBUTING inicial, plantilla scripts |

Objetivos Clave:

- Evitar deuda estructural temprana.
- Asegurar reproducibilidad de build (toolchain fijada).

Pasos sugeridos:

1. Script `check_deps.sh` (usa `cargo metadata`) y falla en ciclos.
2. Pipeline CI: `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `cargo test`.
3. Módulo `hashing::canonical_json` único (no duplicar lógica).
4. Crear `CoreError` / `DomainError` con `thiserror`.
5. Añadir `rust-toolchain` (pin nightly o stable acordado) y caché en CI.
6. Esqueleto `README.md` + `CONTRIBUTING.md`.
7. Primera build limpia confirmando baseline.

Criterios de Cierre (GATE_F0):

- Sin warnings críticos de clippy (nivel deny configurado mínimo).
- Script de dependencias pasa.
- Hashing canónico centralizado (no funciones duplicadas).

---

### F1 – Dominio Inmutable (chem-domain)

| Núcleo                                                                        | Contrato Estabilizado                   | GATE_F1                       | Paralelo Seguro                          |
| ----------------------------------------------------------------------------- | --------------------------------------- | ----------------------------- | ---------------------------------------- |
| Molecule, MoleculeFamily, MolecularProperty, agregados numéricos, invariantes | Hash familia + value_hash reproducibles | 3 ejecuciones → mismos hashes | Catálogo ampliado de futuras propiedades |

Objetivos Clave:

- Garantizar identidad y hash determinista.
- Asegurar insert-only para propiedades.

Pasos sugeridos:

1. `Molecule::new` normaliza InChIKey.
2. `MoleculeFamily::from_iter` fija orden y calcula `family_hash`.
3. Test reproducibilidad (familia idéntica → mismo hash).
4. `MolecularProperty::new` genera `value_hash`.
5. Simular índice de unicidad de inchikey (estructura en tests).
6. Documentar invariantes (`/// INVx:`).
7. Revisión API pública y congelación.

GATE_F1:

- Tests hash determinista pasan.
- No hay mutadores post-freeze.
- value_hash estable (snapshot test).

---

### F2 – Motor Lineal Determinista (chem-core mínimo)

| Núcleo                                                     | Contrato Estabilizado                    | GATE_F2                                                       | Paralelo Seguro                                                 |
| ---------------------------------------------------------- | ---------------------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------- |
| Traits EventStore / FlowRepository + impl memoria + replay | Forma eventos + algoritmo canonical_json | 3 ejecuciones idénticas = misma secuencia eventos/fingerprint | Borrador traits (RetryPolicy / PropertySelectionPolicy) sin uso |

Objetivos Clave:

- Ejecutar pipeline lineal determinista.
- Fingerprint consistente (inputs + params + version interna).

Pasos sugeridos:

1. Definir `StepStatus`.
2. `FlowEngine::next` con validaciones.
3. Emisor centralizado de eventos.
4. Test secuencia idéntica (diff textual).
5. Utilidad `compute_fingerprint`.
6. Verificar neutralidad: sin semántica química.
7. Documentar qué entra/no entra en fingerprint.

GATE_F2:

- Event log idéntico run vs run.
- Fingerprint reproducible.
- Sin semántica dominio en core.

---

### F3 – Persistencia In-Memory Contratada

| Núcleo                                                     | Contrato Estabilizado      | GATE_F3                     | Paralelo Seguro               |
| ---------------------------------------------------------- | -------------------------- | --------------------------- | ----------------------------- |
| Traits EventStore / FlowRepository + impl memoria + replay | Esquema tablas core fijado | Rehidratación DB == memoria | Diseño preliminar esquema SQL |

Objetivos Clave:

- Durabilidad y equivalencia con backend memoria.
- Aislar mapeos dominio↔filas.

Pasos sugeridos:

1. Migración transaccional inicial.
2. Implementar repos Postgres con transacciones atómicas.
3. Test equivalencia (fingerprint final).
4. Índices secuenciales (flow_id, seq).
5. Manejo de errores transitorios (retry simple).
6. Revisión de tipos (UUID, timestamptz).
7. Snapshot esquema documentado.

GATE_F3:

- Replay DB = Replay memoria.
- Sin divergencias en eventos.

---

### F4 – Adaptadores y Steps Iniciales (chem-adapters)

| Núcleo                                                                              | Contrato Estabilizado                              | GATE_F4                          | Paralelo Seguro                |
| ----------------------------------------------------------------------------------- | -------------------------------------------------- | -------------------------------- | ------------------------------ |
| DomainStepAdapter, AcquireMoleculesStep, ComputePropertiesStep stub, Artifact shape | `Artifact {id, kind, hash, payload, metadata_min}` | Hash artifact estable (snapshot) | Bosquejo Normalize / Aggregate |

Objetivos Clave:

- Traducir dominio a artifacts neutrales.
- Validar pipeline Acquire→Compute.

Pasos sugeridos:

1. Trait/función `DomainArtifactEncoder`.
2. AcquireMoleculesStep determinista (dataset sintético).
3. ComputePropertiesStep (sin selección).
4. Test integridad (hash familia referenciado).
5. Ejemplo `examples/basic_workflow.toml`.
6. Verificar no filtrado de datos.
7. Revisión cero tipos dominio en core.

GATE_F4:

- Pipeline lineal produce artifacts hashables reproducibles.
- Artifact shape congelado.

---

### F5 – Persistencia Postgres Mínima (chem-persistence)

| Núcleo                                                                                                    | Contrato Estabilizado      | GATE_F5                     | Paralelo Seguro                |
| --------------------------------------------------------------------------------------------------------- | -------------------------- | --------------------------- | ------------------------------ |
| Migraciones base (EVENT_LOG, WORKFLOW_STEP_EXECUTIONS, WORKFLOW_STEP_ARTIFACTS), mappers, repos concretos | Esquema tablas core fijado | Rehidratación DB == memoria | Índices secundarios (deferred) |

Objetivos Clave:

- Durabilidad y equivalencia con backend memoria.
- Aislar mapeos dominio↔filas.

Pasos sugeridos:

1. Migración transaccional inicial.
2. Implementar repos Postgres con transacciones atómicas.
3. Test equivalencia (fingerprint final).
4. Índices secuenciales (flow_id, seq).
5. Manejo de errores transitorios (retry simple).
6. Revisión de tipos (UUID, timestamptz).
7. Snapshot esquema documentado.

GATE_F5:

- Replay DB = Replay memoria.
- Sin divergencias en eventos.

---

### F6 – Políticas de Selección Básica (chem-policies)

| Núcleo                                                                                      | Contrato Estabilizado           | GATE_F6                                            | Paralelo Seguro                      |
| ------------------------------------------------------------------------------------------- | ------------------------------- | -------------------------------------------------- | ------------------------------------ |
| Trait PropertySelectionPolicy + MaxScore, evento PropertyPreferenceAssigned, preferred flag | Firma choose() + payload evento | Política no altera fingerprint salvo cambio params | Weighted / Consensus (feature gated) |

Objetivos Clave:

- Resolución multi-proveedor determinista.
- Evento auditable con rationale.

Pasos sugeridos:

1. Struct `PropertyCandidate`.
2. Implementación MaxScore (tie-break estable).
3. Emitir evento antes de StepCompleted.
4. Tests: determinismo selección.
5. Parámetros incluidos en fingerprint.
6. Rationale JSON canónico.
7. Feature flags para políticas extra.

GATE_F6:

- Selección estable en entradas iguales.
- Fingerprint sólo cambia con params/política.

---

### F7 – Retry Manual Mínimo

| Núcleo | Contrato Estabilizado | GATE_F7 | Paralelo Seguro |
| ----------------------------------- | --------------------------------------- | ---------------------- | ------------------------------- |Script check_deps.sh (usa cargo metadata) y falla en ciclos.
Pipeline CI: cargo fmt --check, cargo clippy --all-targets --all-features, cargo test.
Módulo hashing::canonical_json único (no duplicar lógica).
Crear CoreError / DomainError con thiserror.
Añadir rust-toolchain (pin nightly o stable acordado) y caché en CI.
Esqueleto README.md + CONTRIBUTING.md.
Primera build limpia confirmando baseline.----------------------------------------------------------------------- | --------------------------------------- | ---------------------- | ------------------------------- |Script check_deps.sh (usa cargo metadata) y falla en ciclos.
Pipeline CI: cargo fmt --check, cargo clippy --all-targets --all-features, cargo test.
Módulo hashing::canonical_json único (no duplicar lógica).
Crear CoreError / DomainError con thiserror.
Añadir rust-toolchain (pin nightly o stable acordado) y caché en CI.
Esqueleto README.md + CONTRIBUTING.md.
Primera build limpia confirmando baseline.----------------------------------------------------------------------- | --------------------------------------- | ---------------------- | ------------------------------- |Script check_deps.sh (usa cargo metadata) y falla en ciclos.
Pipeline CI: cargo fmt --check, cargo clippy --all-targets --all-features, cargo test.
Módulo hashing::canonical_json único (no duplicar lógica).
Crear CoreError / DomainError con thiserror.
Añadir rust-toolchain (pin nightly o stable acordado) y caché en CI.
Esqueleto README.md + CONTRIBUTING.md.
Primera build limpia confirmando baseline.----------------------------------------------------------------------- | --------------------------------------- | ---------------------- | ------------------------------- |
| RetryPolicy (should_retry), transición Failed→Pending, retry_count memoria, evento RetryScheduled opcional | Semántica retry (no altera fingerprint) | Backoff diseño inicial | Borrador estrategia exponencial |

Objetivos Clave:

- Reintentos sin persistencia de errores.
- Idempotencia de fingerprint.

Pasos sugeridos:

1. Campo `retry_count` en StepSlot.
2. `FlowEngine::retry(step_id)`.
3. Evento RetryScheduled (si < max).
4. Test: exceder max rechaza.
5. Nuevos artifacts generan nuevos IDs (no colisión hash).
6. Métrica interna retries.
7. Documentar semántica.

GATE_F7:

- Reintento no cambia fingerprint.
- No artifacts duplicados.

---

### F8 – Persistencia Extendida de Errores

| Núcleo                                                             | Contrato Estabilizado            | GATE_F8                     | Paralelo Seguro        |
| ------------------------------------------------------------------ | -------------------------------- | --------------------------- | ---------------------- |
| Migración STEP_EXECUTION_ERRORS, persistir retry_count/max_retries | Esquema errores + attempt_number | Rehidratación DB == memoria | Métricas error (luego) |

Objetivos Clave:

- Auditoría granular de fallos.
- Base para políticas avanzadas.

Pasos sugeridos:

1. Migración tabla errores + índice (step_id, attempt_number).
2. Persistir retry_count tras transición.
3. Insert por fallo (Validation/Runtime).
4. Test reconstrucción timeline.
5. Consulta verificación counts.
6. Clasificación error_class / transient.
7. Documentación formato details JSON.

GATE_F8:

- Timeline errores exacto.
- RetryPolicy puede leer clasificación.

---

### F9 – Branching Determinista

| Núcleo                                                                                        | Contrato Estabilizado                      | GATE_F9                           | Paralelo Seguro  |
| --------------------------------------------------------------------------------------------- | ------------------------------------------ | --------------------------------- | ---------------- |
| Tabla WORKFLOW_BRANCHES, API branch(from_step), evento BranchCreated (divergence_params_hash) | Modelo branch + invariantes fork Completed | Árbol raíz + 2 ramas reproducible | CLI listar ramas |

Objetivos Clave:

- Fork reproducible sin duplicar histórico anterior.
- Comparación de fingerprints post-divergencia.

Pasos sugeridos:

1. Migración WORKFLOW_BRANCHES.
2. Implementar clon parcial (slots ≤ N).
3. Copiar sólo eventos hasta N.
4. divergence_params_hash (hash canónico).
5. Test convergencia sin cambios params.
6. CLI `branch --from-step`.
7. Documentar invariantes (no branch sobre Failed/Pending).

GATE_F9:

- Ramas reproducibles con árbol consistente.
- Sin duplicación de eventos futuros.

---

### F10 – Inyección Compuesta + Human Gate

| Núcleo                                                                                                      | Contrato Estabilizado                                | GATE_F10                           | Paralelo Seguro      |
| ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------------------------- | -------------------- |
| ParamInjector trait, CompositeInjector, estado AwaitingUserInput, eventos UserInteractionRequested/Provided | Orden merge estable (base→injectors→overrides→human) | Nuevos inyectores por ArtifactKind | UI mínima aprobación |

Objetivos Clave:

- Parametrización extensible determinista.
- Interacción humana sin contaminar fingerprint salvo overrides.

Pasos sugeridos:

1. Definir orden merge fijo.
2. CompositeInjector determinista + test.
3. Extender state machine (AwaitingUserInput).
4. `resume_user_input(...)`.
5. Validar schema input humano.
6. decision_hash (rationale).
7. Test fingerprint estable con/ sin gate (mismos overrides).

GATE_F10:

- Fingerprint sólo cambia con overrides.
- Reanudación segura y reproducible.

---

### F11 – Políticas Avanzadas / Branching Automático

| Núcleo                                                                 | Contrato Estabilizado                        | GATE_F11                                      | Paralelo Seguro                    |
| ---------------------------------------------------------------------- | -------------------------------------------- | --------------------------------------------- | ---------------------------------- |
| PolicyEngine (decide_branch / decide_retry), persist divergence_params | Firma PolicyEngine + shape divergence_params | Branch sólo si fingerprint proyectado diverge | Heurísticas experimentales (flags) |

Objetivos Clave:

- Decisiones automáticas reproducibles.
- Divergencia justificada y registrada.

Pasos sugeridos:

1. Interface PolicyEngine.
2. Fingerprint hipotético (dry-run).
3. Criterios (threshold / hash diff).
4. Evento BranchCreated por autopolicy.
5. Test exactamente una rama nueva.
6. Métrica branches automáticos.
7. Feature flag `branching_auto`.

GATE_F11:

- Branching automático sin falsos positivos.
- Divergence_params trazables.

---

### F12 – Normalización de Agregados

| Núcleo                                                                      | Contrato Estabilizado                        | GATE_F12                               | Paralelo Seguro                  |
| --------------------------------------------------------------------------- | -------------------------------------------- | -------------------------------------- | -------------------------------- |
| Tabla FAMILY_AGGREGATE_NUMERIC + refactor Step agregados (JSONB + numérico) | Esquema numérico (family_id, aggregate_name) | Consultas sin parseo JSON equivalentes | Diseño tabla DISTRIBUTION futuro |

Objetivos Clave:

- Consultas eficientes.
- Mantener compatibilidad JSON.

Pasos sugeridos:

1. Migración tabla + índice compuesto.
2. Escritura dual (transacción).
3. Test hash agregados equivalentes.
4. Ejemplo consulta optimizada.
5. Verificar rollback consistente.
6. Documentar naming agregados.

GATE_F12:

- Escritura dual estable.
- Hash no cambia vs versión previa.

---

### F13 – Observabilidad y Tooling

| Núcleo                                                                                              | Contrato Estabilizado                  | GATE_F13            | Paralelo Seguro                    |
| --------------------------------------------------------------------------------------------------- | -------------------------------------- | ------------------- | ---------------------------------- |
| Trait Metrics + logging estructurado (flow_id, branch_id), scripts check_deps / regenerate_diagrams | API métrica estable (Noop por defecto) | Exporter Prometheus | Integración tracing spans completa |

Objetivos Clave:

- Instrumentar sin vendor lock.
- No afectar determinismo.

Pasos sugeridos:

1. Trait `MetricsSink`.
2. Noop + simple Prometheus exporter.
3. Spans `step.run`.
4. Script diagrams a SVG/PNG.
5. Test feature metrics no altera fingerprint.
6. Panel ejemplo docs.
7. Job CI separado para lint arquitectura.

GATE_F13:

- Métricas desactivables sin recompilar core (features).
- Logs correlacionados.

---

### F14 – Hardening y Caching

| Núcleo                                                                                           | Contrato Estabilizado                         | GATE_F14                 | Paralelo Seguro                      |
| ------------------------------------------------------------------------------------------------ | --------------------------------------------- | ------------------------ | ------------------------------------ |
| Cache fingerprint→artifact (trait), backend alterno memoria, firma encadenada eventos (opcional) | Semántica cache (miss = comportamiento igual) | Cache distribuida futura | Firma criptográfica fuerte posterior |

Objetivos Clave:

- Optimizar sin alterar semántica.
- Base para validación criptográfica.

Pasos sugeridos:

1. Trait `ArtifactCache` (get/put).
2. Implementación in-memory.
3. ChainSignature (prev_sig + event_hash).
4. Feature flag `event_signing`.
5. Test: cache hit evita recomputar (hash artifact igual).
6. Estrategia invalidación (version bump).
7. Revisión seguridad (firma fuera de fingerprint).

GATE_F14:

- Activar cache no cambia resultados (sólo latencia).
- Firma opcional validada encadenada.

---
