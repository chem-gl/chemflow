# Sección 23 - Organización de Carpetas (Propuesta Rust – Versión Ampliada)

Objetivo: estructurar el workspace para reflejar límites arquitectónicos (Dominio, Core, Adaptación, Persistencia, Políticas, Proveedores, Integraciones, CLI) maximizando: desacoplamiento, testabilidad, reproducibilidad y facilidad de evolución incremental descrita en la sección 24. Se añade un crate específico para Proveedores externos y se enriquecen capas de pruebas (benchmarks, fuzzing) y tooling.

### 23.1 Estructura de Carpetas (Workspace Rust)

```
chemflow-rust/
├─ Cargo.toml                  (workspace raíz)
├─ rust-toolchain
├─ README.md
├─ crates/
│  ├─ chem-domain/             (Entidades químicas puras + invariantes)
│  │  ├─ src/
│  │  │  ├─ molecule.rs
│  │  │  ├─ molecule_family.rs
│  │  │  ├─ properties/
│  │  │  │  ├─ kind.rs
│  │  │  │  ├─ value.rs
│  │  │  │  ├─ provenance.rs
│  │  │  │  └─ selection_policy.rs
│  │  │  ├─ aggregates/
│  │  │  │  ├─ numeric.rs
│  │  │  │  ├─ categorical.rs
│  │  │  │  └─ projection.rs
│  │  │  ├─ invariants/
│  │  │  │  ├─ molecule_invariants.rs
│  │  │  │  ├─ family_invariants.rs
│  │  │  │  └─ property_invariants.rs
│  │  │  └─ lib.rs
│  │  ├─ tests/
│  │  │  ├─ molecule_tests.rs
│  │  │  ├─ family_tests.rs
│  │  │  └─ property_tests.rs
│  │  └─ Cargo.toml
│  ├─ chem-core/               (Motor genérico sin semántica química)
│  │  ├─ src/
│  │  │  ├─ engine/
│  │  │  │  ├─ flow_engine.rs
│  │  │  │  ├─ recovery.rs
│  │  │  │  ├─ branching.rs
│  │  │  │  ├─ policy_engine.rs
│  │  │  │  └─ validator.rs
│  │  │  ├─ model/
│  │  │  │  ├─ flow_definition.rs
│  │  │  │  ├─ flow_instance.rs
│  │  │  │  ├─ step_slot.rs
│  │  │  │  ├─ step_definition.rs
│  │  │  │  ├─ artifact.rs
│  │  │  │  ├─ events.rs
│  │  │  │  └─ execution_context.rs
│  │  │  ├─ params/
│  │  │  │  ├─ injector.rs
│  │  │  │  ├─ composite_injector.rs
│  │  │  │  ├─ human_gate.rs
│  │  │  │  └─ merge_strategies.rs
│  │  │  ├─ retry/
│  │  │  │  ├─ retry_policy.rs
│  │  │  │  ├─ backoff.rs
│  │  │  │  └─ error_classification.rs
│  │  │  ├─ hashing/
│  │  │  │  ├─ fingerprint.rs
│  │  │  │  ├─ canonical_json.rs
│  │  │  │  └─ hash_utils.rs
│  │  │  ├─ cache/
│  │  │  │  ├─ cache_trait.rs
│  │  │  │  └─ memory_cache.rs
│  │  │  ├─ state_machine.rs
│  │  │  └─ lib.rs
│  │  ├─ tests/
│  │  │  ├─ engine_tests.rs
│  │  │  ├─ state_machine_tests.rs
│  │  │  └─ fingerprint_tests.rs
│  │  └─ Cargo.toml
│  ├─ chem-adapters/           (ACL dominio ↔ core; empaquetado a artifacts)
│  │  ├─ src/
│  │  │  ├─ domain_step_adapter.rs
│  │  │  ├─ chem_artifact_encoder.rs
│  │  │  ├─ steps/
│  │  │  │  ├─ acquire.rs
│  │  │  │  ├─ compute_properties.rs
│  │  │  │  ├─ normalize_properties.rs
│  │  │  │  ├─ aggregate.rs
│  │  │  │  ├─ filter.rs
│  │  │  │  ├─ report.rs
│  │  │  │  └─ human_gate_step.rs
│  │  │  ├─ provider_adapters/
│  │  │  │  ├─ molecule_provider_adapter.rs
│  │  │  │  ├─ property_provider_adapter.rs
│  │  │  │  └─ data_provider_adapter.rs
│  │  │  └─ lib.rs
│  │  ├─ tests/
│  │  │  ├─ adapter_tests.rs
│  │  │  ├─ step_tests.rs
│  │  │  └─ integration_tests.rs
│  │  └─ Cargo.toml
│  ├─ chem-persistence/        (Repos, mapeos y migraciones Postgres)
│  │  ├─ src/
│  │  │  ├─ repositories/
│  │  │  │  ├─ flow_repository.rs
│  │  │  │
| chem-core        | Orquestación genérica (eventos, branching, retry)   | Reemplazable / potencial OSS genérico |
| chem-adapters    | Traducción dominio ↔ artifacts neutrales            | Protege Core de cambios semánticos    |
| chem-persistence | Persistencia y migraciones                          | Backend intercambiable                |
| chem-policies    | Políticas versionadas (selección, retry, branching) | Experimentación aislada               |
| chem-providers   | Integración con proveedores externos (IO pesado)    | Aísla dependencias y latencia         |
| chem-cli         | Operación interactiva / scripts                     | Distribución sencilla                 |
| chem-infra       | Observabilidad, HPC, storage, alertas               | Dependencias pesadas encapsuladas     |

```

### 23.3 Dependencias Permitidas (Reglas de Capa)

```
chem-domain -> (ninguna interna)
chem-core -> chem-domain
chem-policies -> chem-core, chem-domain
chem-adapters -> chem-core, chem-domain
chem-persistence -> chem-core, chem-domain
chem-providers -> chem-core, chem-domain, chem-adapters
chem-infra -> chem-core, chem-adapters, chem-policies
chem-cli -> chem-core, chem-adapters, chem-persistence, chem-policies, chem-providers
tests/\* -> cualquiera (activando features)
```

Validación automática recomendada: script `check_deps.sh` usando `cargo metadata` + (opcional) `cargo-deny`.

### 23.4 Features y Flags Sugeridos (Cargo)

Workspace (añade librerías para observabilidad y async, manteniendo compatibilidad con lo ya descrito en secciones previas):

```toml
[workspace]
members = [
    "crates/chem-domain",
    "crates/chem-core",
    "crates/chem-adapters",
    "crates/chem-persistence",
    "crates/chem-policies",
    "crates/chem-providers",
    "crates/chem-cli",
    "crates/chem-infra"
]

[workspace.package]
edition = "2021"
rust-version = "1.70"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
tokio = { version = "1.0", features = ["full"] }
diesel = { version = "2", features = ["postgres", "r2d2", "chrono", "serde_json", "uuid"] }
diesel_migrations = { version = "2", features = ["postgres"] }
// Nota: Se reemplazó sqlx por Diesel para alineación con Fase 3.
```

Ejemplo features `chem-core` (extiende lista previa añadiendo caching y branching explícito):

```toml
[features]
default = ["fingerprint", "retry", "caching"]
fingerprint = []
retry = []
caching = []
user_interaction = []
branching = []
```

Directrices: no activar experimental (branching, caching) en `default` hasta estabilizar; separar features facilita compilaciones mínimas para pruebas del dominio.

### 23.5 Migración Incremental desde la Estructura Actual

1. Inicializar workspace y mover bin actual a `chem-cli` dejando `chem-core` como `lib` (mantener compilación verde).
2. Extraer dominio puro a `chem-domain` (tipos + invariantes) hasta que `chem-core` no dependa de semántica química.
3. Crear `chem-adapters` moviendo lógica de empaquetado (steps concretos) fuera del Core.
4. Introducir `chem-persistence` (repos + migraciones mínimas) e inyectar interfaces en el motor.
5. Añadir `chem-policies` abstra-yendo selección de propiedades y retry simple (interfaz estable).
6. Extraer proveedores a `chem-providers` (mantener Stubs si aún no hay integración real) y adaptar Steps a usar registry.
7. Incorporar `chem-infra` (observabilidad básica + HPC simulada) tras estabilizar eventos.
8. Activar features opcionales (branching, caching, user_interaction) una vez completadas tablas sección 12.
9. Añadir capas de pruebas extendidas (benchmarks, fuzz) tras asegurar determinismo base.

Cada paso: commit pequeño + tests verdes + verificación de dependencias con `check_deps.sh`.

### 23.6 Tests Estratificados (Ampliados)

| Nivel          | Ubicación                   | Objetivo                                     |
| -------------- | --------------------------- | -------------------------------------------- |
| Unidad dominio | `chem-domain/tests`         | Invariantes (hash, preferred único)          |
| Unidad core    | `chem-core/tests`           | State machine, fingerprint, branching básica |
| Adaptadores    | `chem-adapters/tests`       | Mapping domain→artifact                      |
| Persistencia   | `chem-persistence/tests`    | Transacciones, idempotencia, migraciones     |
| Políticas      | `chem-policies/tests`       | Selección / retry / branching heurístico     |
| Proveedores    | `chem-providers/tests`      | Registro, fallback, simulación latencias     |
| Integración    | `tests/integration`         | Branching + retry + recovery + human gate    |
| End‑to‑End     | `tests/smoke/end_to_end.rs` | Pipeline completo determinista               |
| Benchmarks     | `tests/benchmarks`          | Hot paths (fingerprint, ejecución, DB)       |
| Fuzzing        | `tests/fuzz`                | Robustez parámetros / payload artifacts      |

### 23.7 Observabilidad, Tooling y Scripts

| Script                   | Función                                     |
| ------------------------ | ------------------------------------------- |
| `dev_db.sh`              | Levantar BD desarrollo (Docker/local)       |
| `deploy_migrations.sh`   | Aplicar migraciones en orden                |
| `lint_all.sh`            | `cargo clippy --all-targets --all-features` |
| `test_all.sh`            | Ejecutar suite completa (sin benches)       |
| `coverage.sh`            | Generar cobertura (p.ej. tarpaulin)         |
| `check_deps.sh`          | Validar reglas de capa                      |
| `regenerate_diagrams.sh` | Render Mermaid → SVG/PNG                    |

### 23.8 Principios de Aceptación

1. Cero ciclos entre crates (verificable automáticamente).
2. `chem-core` ignora enumeraciones químicas concretas (sólo traits neutrales).
3. Cambios en heurísticas (políticas) no recompilan Core si no se usan features adicionales.
4. Un pipeline completo se arma orquestando crates; no se modifica código del motor para añadir dominios.
5. Dependencias externas (IO / HPC / proveedores) encapsuladas fuera de Core (latencia controlada y fácil mocking).
6. Fingerprints permanecen deterministas bajo activación de features opcionales (tests de regresión dedicados).
7. Los cambios en las políticas de selección de propiedades y en las estrategias de reintento no deben requerir cambios en el código del núcleo, siempre que se mantenga la misma firma de trait.

### 23.9 Próximos Pasos Opcionales

- Añadir `deny.toml` (licencias / vulnerabilidades).
- Publicar `chem-core` como crate independiente (cuando API se estabilice).
- Integrar `cargo-nextest` para acelerar CI.
- Añadir `criterion` para benchmarks formales y registrar tendencias.
- Implementar plugin simple de lint de arquitectura (script que falle si se viola matriz de dependencias).

