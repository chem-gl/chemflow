
# ChemFlow — Documentación y Checklist de Verificación

Este archivo contiene una lista de verificación (120 checks) destinada a
validar que el proyecto `chemflow` está en orden en múltiples dimensiones
(compilación, tests, calidad de código, seguridad, documentación, infra,
CI, migraciones, etc.). Al final se incluyen 30 acciones recomendadas para
mejorar la base de código y el flujo de trabajo del equipo.

Instrucciones: marque cada check según corresponda y use las acciones
recomendadas para priorizar mejoras.

## Lista para entender el proyecto (120 ítems)

Esta sección reemplaza la verificación técnica por una guía de lectura: 120
puntos que una persona debe revisar para comprender la arquitectura, dominio
y las decisiones clave del proyecto. No son pruebas de funcionamiento sino
orientaciones para estudiar el código y su diseño.

1. Leer el `README.md` raíz para entender el objetivo general del proyecto.
2. Revisar `documentation/diagramas-final.md` para la visión de arquitectura.
3. Abrir `documentation/tareas/tarea1.md` para el diseño del dominio.
4. Leer `crates/chem-domain/src/lib.rs` para ver la API pública del dominio.
5. Revisar los modelos `Molecule`, `MoleculeFamily`, `MolecularProperty`.
6. Entender cómo se normaliza y valida un `Molecule` (InChI/InChIKey/SMILES).
7. Ver dónde y cómo se calcula `family_hash` en `MoleculeFamily`.
8. Ver la intención de `value_hash` en propiedades y su propósito.
9. Revisar `family_property.rs` para entender propiedades a nivel de familia.
10. Revisar `molecular_property.rs` para ver propiedades atómicas/moleculares.
11. Leer `crates/chem-core` para conocer el motor/contratos core del flujo.
12. Identificar los traits `EventStore` y `FlowRepository` en `chem-core`.
13. Seguir el flujo de eventos: qué es un `FlowEventKind` y sus variantes.
14. Ver la estructura `ExecutionContext` y los datos que pasan a un Step.
15. Revisar `step` y `StepDefinition` para entender cómo se implementan steps.
16. Entender `StepRunResult` y las posibles señales (`StepSignal`).
17. Leer sobre fingerprints: `StepFingerprintInput` y cuándo se calculan.
18. Revisar `hashing::canonical_json` para comprender serialización determinista.
19. Revisar dónde se llama `hash_str` y cómo se usan los hashes.
20. Identificar el punto de entrada principal del engine (ej. `FlowEngine`).
21. Ver cómo se construye una definición de flujo (`build_flow_definition`).
22. Revisar los adapters en `crates/chem-adapters` (acquire/compute/encoder).
23. Entender cómo los adapters traducen domain -> artifacts neutrales.
24. Revisar `DomainArtifactEncoder` para ver la representación de payloads.
25. Ver ejemplos en `examples/` para un recorrido práctico del API.
26. Abrir `crates/chem-engine` para entender la integración con Python/RDKit.
27. Leer `chem-engine/src/core.rs` para ver la inicialización PyO3 y wrapper.
28. Revisar pruebas unitarias en `chem-engine` para ejemplos de uso.
29. Identificar dependencias nativas (RDKit) y requisitos del sistema.
30. Revisar `crates/chem-persistence` para ver cómo se persisten eventos.
31. Leer `crates/chem-persistence/src/schema.rs` para entender tablas DB.
32. Abrir `migrations/` en ese crate para ver DDL histórico y cambios.
33. Revisar `migrations.rs` para entender cómo se ejecutan migraciones embebidas.
34. Lee `pg` implementaciones: `PgEventStore` y `PgFlowRepository`.
35. Entender la paridad entre InMemory y Pg: ¿qué se espera de cada uno?
36. Revisar `tests/` de `chem-persistence` para ver casos de integración.
37. Ver `test_support.rs` para entender cómo se crea el pool de pruebas.
38. Entender cómo los tests saltan cuando falta `DATABASE_URL`.
39. Leer `event_roundtrip_variants.rs` para ver transporte de variantes de evento.
40. Revisar `seq_integrity.rs` y por qué `seq` por `flow_id` importa.
41. Revisar `engine_fingerprint.rs` para comparar fingerprint entre backends.
42. Leer `validate_f8.rs` para entender la persistencia extendida de errores.
43. Ver `error_persistence.rs` para ejemplos de captura y clasificación de errores.
44. Revisar `policy_event_translation.rs` para entender eventos reservados.
45. Revisar `event_parity.rs` para ver pruebas de paridad memoria vs pg.
46. Leer `stress.rs` para entender casos extremos simulados por el equipo.
47. Revisar `minimal_pool.rs` para comprender ciclos de creación/destrucción.
48. Leer `teardown.rs` para prácticas de limpieza/teardown en tests.
49. Localizar y entender macros útiles (`typed_artifact!`, `typed_step!`).
50. Investigar cómo se definen artefactos tipados y su uso en ejemplos.
51. Revisar `crates/chem-policies` para ver políticas de selección (MaxScore).
52. Entender `PropertyCandidate`, `SelectionParams`, `SelectionDecision`.
53. Ver tests de `chem-policies` que demuestran determinismo y desempates.
54. Leer `crates/chem-adapters/steps` para ver steps concretos implementados.
55. Revisar `chem-adapters/artifacts.rs` para shape y serialización de artifacts.
56. Revisar cómo se codifican/decodifican artifacts (`encoder.rs`).
57. Localizar la implementación de `FlowEngine::retry` si existe o su esqueleto.
58. Revisar documentación en `documentation/tareas` para entender milestones.
59. Leer `diagramas.md` y `diagramas-final.md` para la visión de componentes.
60. Revisar `documentation/snapshots` para esquemas SQL históricos.
61. Ver `postgress-docker/compose.yaml` para reproducir entorno Postgres local.
62. Ejecutar mentalmente (o localmente) `dev_db.sh` para ver setup DB recomendado.
63. Revisar `setup-python.sh` para ver cómo preparar entorno RDKit/Python.
64. Identificar dónde se generan y consumen UUIDs (convención v4 esperada).
65. Revisar `src/hashing/canonical_json.rs` y sus tests para comportamiento esperado.
66. Leer los tests de `chem-domain` para ver ejemplos de construcción de modelos.
67. Revisar `src/errors` y entender las convenciones de error del workspace.
68. Ver `thiserror` usage y cómo se mapean errores externos a errores de dominio.
69. Revisar `crates/chem-providers` como ejemplo de proveedor / stub.
70. Revisar `src/lib.rs` principal (workspace-level) para re-exports y utilidades.
71. Localizar dónde están las funciones públicas de conveniencia (build_flow_definition).
72. Revisar `examples/basic_workflow.toml` para ver configuración de ejemplo.
73. Entender la convención de naming y estructura de carpetas del repo.
74. Revisar `check_deps.sh`, `lint_all.sh`, `test_all.sh` para comandos comunes.
75. Ver `coverage.sh` y `cobertura.xml` para cómo se mide cobertura.
76. Revisar `CONTRIBUTING.md` para el flujo de trabajo de contribuciones.
77. Localizar `Dockerfile` u otro contenedor recomendado (si existe).
78. Revisar `run.sh` para atajos de ejecución/local development.
79. Identificar dependencias clave en `Cargo.toml` (diesel, pyo3, serde, uuid).
80. Revisar `rust-toolchain` para la versión Rust esperada por el proyecto.
81. Explorar `crates/*/README.md` para documentación específica de cada crate.
82. Ver si existe `CHANGELOG.md` o historial de releases.
83. Confirmar la licencia del proyecto en `LICENSE` (si existe).
84. Revisar convenciones de commit y branching (ej. `main` como rama por defecto).
85. Buscar `TODO` y `FIXME` en el código para áreas señaladas por autores.
86. Revisar usos de `unsafe` o FFI y entender por qué aparecen.
87. Revisar cómo se manejan la configuración y variables de entorno (`DbConfig`).
88. Leer `crates/chem-persistence/src/config.rs` para políticas de pool y .env.
89. Revisar `test_support::with_pool` y entender cómo los tests comparten recursos.
90. Revisar `crates/chem-core/tests` y `crates/*/tests` para ejemplos de integración.
91. Localizar pruebas que sirven como especificación del comportamiento.
92. Revisar cómo se documentan invariantes importantes en comentarios `///`.
93. Identificar qué módulos exportan la API pública y cuáles son internos.
94. Revisar `src/hashing/mod.rs` para re-exports y puntos de integración.
95. Ver cómo se serializa/deserializa `FlowEventKind` y su mapeo a la DB.
96. Entender la estrategia de deduplicación de artifacts y su justificación.
97. Revisar el diseño de `workflow_step_artifacts` en `schema.rs`.
98. Revisar tests que validan roundtrip JSON <-> estructuras tipadas.
99. Revisar el manejo de timestamps (`ts`) y zonas horarias (timestamptz).
100. Entender cómo se generan y representan `attempt_number` para step errors.
101. Buscar ubicaciones donde se calcula el fingerprint del flujo completo.
102. Revisar las políticas (F6) y cómo se integran con el engine (eventos de preference).
103. Leer `documentation/tareas/tarea7.md` para la semántica de retry propuesta.
104. Revisar `tarea9.md` para entender branching determinista y su alcance.
105. Revisar `tarea10.md` para inyección compuesta y human gate design.
106. Ver ejemplos o tests de `CompositeInjector` si existen.
107. Localizar el código que emite eventos reservados y su traducción a DB.
108. Revisar cómo se representan y recuperan `metadata` en artifacts.
109. Ver cómo se versionan las schemas o decisiones (schema_version en artifacts).
110. Revisar cómo se documentan o almacenan `provenance` para familias/moléculas.
111. Buscar utilidades de debugging y trazado (e.g. logs, eprintln en tests).
112. Revisar uso de `once_cell::Lazy` y `OnceLock` para recursos singleton.
113. Entender la estrategia de testing local vs CI (qué se ejecuta en CI).
114. Revisar ejemplos de uso de `typed_step!` en `src/main.rs` y `main.rs` raíz.
115. Revisar `crates/chem-adapters/tests` o integraciones para casos reales.
116. Revisar la comunicación entre `chem-core` y `chem-persistence` (interfaces públicas).
117. Ver la política de expiración/retención para artifacts (si está documentada).
118. Revisar si existe manejo de migraciones en runtime y cómo se aplica en despliegues.
119. Revisar notas de rendimiento o perfiles (si existen) para hotspots conocidos.
120. Hacer una pasada rápida por commits recientes para entender cambios y prioridades.

## 30 Acciones recomendadas para mejorar el código y el flujo

1. Priorizar y añadir tests unitarios para las funciones públicas faltantes en `chem-core` y `chem-domain`.
2. Añadir un pipeline CI que ejecute `cargo fmt`, `cargo clippy` y `cargo test --all --workspace` en cada PR.
3. Documentar los requisitos de sistema para `chem-engine` (RDKit, Python) y proporcionar Dockerfile o imágenes de desarrollo.
4. Añadir un `Makefile` o `justfile` con comandos comunes (build/test/run/migrate).
5. Migrar la carga de `.env` fuera de código crítico y usar una estrategia de configuración centralizada.
6. Reforzar el manejo de errores en `chem-engine` para no panicar en inicialización PyO3.
7. Añadir tests de integración que ejecuten un pequeño flujo end-to-end en modo memoria y otro con Postgres (si DATABASE_URL presente).
8. Añadir benchmarks micro para hashing y canonical JSON para verificar performance.
9. Mejorar la documentación de las macros `typed_artifact!` y `typed_step!` con ejemplos.
10. Añadir validaciones y asserts en constructors (`new`) para invariantes (e.g., non-empty IDs).
11. Reemplazar `println!` de demo por logging configurado (e.g., `tracing`).
12. Añadir CI job que ejecute migraciones y tests contra una base Postgres en contenedor.
13. Establecer políticas de versiones semánticas para crates y documentar breaking changes.
14. Crear una suite de smoke tests de compatibilidad entre memoria y Postgres (paridad).
15. Añadir tests que validen la continuidad de `seq` bajo concurrencia controlada.
16. Auditar y actualizar dependencias vulnerables o no mantenidas.
17. Añadir un archivo `ISSUE_TEMPLATE.md` y `PULL_REQUEST_TEMPLATE.md` para estandarizar contribuciones.
18. Añadir documentación de arquitectura mínima (diagrama en PNG/SVG + README resumido).
19. Extraer y centralizar utilidades comunes de hashing y JSON canónico en un crate utilitario bien versionado.
20. Añadir validaciones de esquema JSON (si aplica) para `payload` en eventos antes de persistir.
21. Implementar un modo de pruebas que use bases de datos efímeras (testcontainers) para reproducibilidad.
22. Añadir logging estructurado y métricas básicas (counters para eventos, latencias).
23. Añadir checks de lint en formato `pre-commit` hooks (rustfmt, clippy).
24. Generar documentación (cargo doc) y publicarla en GitHub Pages o similar.
25. Escribir una guía de contribución específica para nuevos desarrolladores (setup local paso a paso).
26. Añadir tests de propiedad (proptest) para hashing/canonicalization y ordenamientos.
27. Asegurar que los elementos que interactúan con PyO3 tengan mocks para CI sin RDKit.
28. Introducir validaciones en runtime para entradas críticas (e.g., longitudes máximas de strings JSON).
29. Agregar un script de verificación de migraciones pendientes en CI antes de desplegar.
30. Programar una revisión de código completa y etiquetar issues para las 10 mejoras más críticas.

---

Resumen: esta lista está pensada como punto de partida exhaustivo para revisar
calidad, reproducibilidad y resiliencia del proyecto; las acciones al final son
priorizables y de bajo riesgo.

## Árbol de archivos prioritario (no .md)

Lista jerarquizada de archivos y rutas (sin entradas `.md`) que conviene revisar
para entender la estructura, las piezas críticas y el flujo de datos del
proyecto. Ordenadas por importancia sugerida.

1. Cargo.toml (raíz) — dependencias y workspace
2. Cargo.lock — versiones reproducibles de dependencias
3. rust-toolchain — versión de Rust esperada
4. src/main.rs — binario principal / orquestador (puntos de integración)
5. src/lib.rs — utilidades/exports del workspace
6. src/errors/core_error.rs
7. src/errors/domain_error.rs
8. src/hashing/canonical_json.rs — canonicalización JSON y tests
9. crates/chem-core/Cargo.toml
10. crates/chem-core/src/lib.rs — contratos `FlowEngine`, `EventStore`, `FlowRepository`
11. crates/chem-core/src/model/** (modelos clave como Artifact, ExecutionContext)
12. crates/chem-core/src/step/** (definición de Step, StepRunResult, StepSignal)
13. crates/chem-domain/Cargo.toml
14. crates/chem-domain/src/lib.rs
15. crates/chem-domain/src/molecule.rs
16. crates/chem-domain/src/molecule_family.rs
17. crates/chem-domain/src/molecular_property.rs
18. crates/chem-domain/src/family_property.rs
19. crates/chem-engine/Cargo.toml
20. crates/chem-engine/src/lib.rs
21. crates/chem-engine/src/core.rs — inicialización PyO3 / RDKit wrapper
22. crates/chem-engine/python/rdkit_wrapper.py — código Python integrado
23. crates/chem-adapters/Cargo.toml
24. crates/chem-adapters/src/encoder.rs
25. crates/chem-adapters/src/artifacts.rs
26. crates/chem-adapters/src/steps/** (acquire, compute, policy_demo)
27. crates/chem-persistence/Cargo.toml
28. crates/chem-persistence/src/lib.rs
29. crates/chem-persistence/src/config.rs — carga de .env y DbConfig
30. crates/chem-persistence/src/migrations.rs — runner de migraciones embebidas
31. crates/chem-persistence/src/schema.rs — tablas Diesel y tipos (event_log, artifacts, errors)
32. crates/chem-persistence/src/error.rs — mapeo Diesel -> PersistenceError
33. crates/chem-persistence/pg/** — implementaciones Postgres (PgEventStore, PgFlowRepository, pool builders)
34. crates/chem-persistence/migrations/** (archivos SQL) — revisar orden y DDL
35. crates/chem-persistence/tests/test_support.rs — construcción de pool de pruebas
36. crates/chem-persistence/tests/*.rs — tests de paridad, integridad y stress (por ejemplo: seq_integrity.rs, stress.rs)
37. crates/chem-policies/Cargo.toml
38. crates/chem-policies/src/lib.rs — reglas, tie-breakers y serialización (MaxScore)
39. crates/chem-providers/Cargo.toml
40. crates/chem-providers/src/lib.rs — stubs/providers
41. crates/chem-adapters/tests and crates/chem-core/tests — ejemplos de integración
42. examples/basic_workflow.toml — ejemplo de configuración de flujo
43. postgress-docker/compose.yaml — contenedor Postgres usado para pruebas locales
44. scripts/dev_db.sh, scripts/setup-python.sh — scripts de entorno (DB, RDKit)
45. run.sh, check_deps.sh, lint_all.sh, test_all.sh — scripts de utilidad y verificación
46. target/ (ignorado en VCS normalmente) — artefactos de build (no revisar en VCS)
47. Cargo.toml de cada crate en `crates/*` — revisar versiones y features por crate
48. crates/*/src/**/*.rs que implementan lógica pública (priorizar `lib.rs`, `core.rs`, `pg/*.rs`)
49. documentation/snapshots/schema_f3.sql, schema_f5.sql, schema_f6.sql — dumps DDL de referencia
50. Any `*.rs` en la raíz del workspace `src/` o en crates que contengan `main`/`lib` expuestos

Notas rápidas:
- Prioridad indicada: 1–10 = crítico para entender dominio y core; 11–30 = persistencia e integración; 31–50 = adaptadores, infra y utilidades.
- Evitar revisar primero archivos `target/` o binarios; centrarse en crates y `src`.
- Si quieres puedo convertir esta lista en un checklist interactivo o generar issues para las rutas más críticas.
