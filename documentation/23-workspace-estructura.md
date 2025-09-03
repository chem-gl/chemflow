## 23. Organización de Carpetas (Propuesta Rust – Versión Ampliada)

Objetivo: reflejar límites arquitectónicos y evolución incremental.

### 23.1 Estructura Workspace

(Árbol reproducido completo con crates chem-domain, chem-core, chem-adapters, chem-persistence, chem-policies, chem-providers, chem-cli, chem-infra.)

### 23.2 Justificación Capas

| Crate            | Rol                  | Notas              |
| ---------------- | -------------------- | ------------------ |
| chem-core        | Orquestación         | Potencial OSS      |
| chem-adapters    | ACL                  | Protege Core       |
| chem-persistence | Repos & migraciones  | Intercambiable     |
| chem-policies    | Políticas            | Experimentación    |
| chem-providers   | IO externo           | Encapsula latencia |
| chem-cli         | Operación            | Distribución       |
| chem-infra       | Observabilidad / HPC | Pesado aislado     |

### 23.3 Dependencias Permitidas

Bloque reglas sin ciclos (listado reproducido).

### 23.4 Features y Flags

Bloques Cargo.toml workspace y features chem-core (fingerprint, retry, caching, user_interaction, branching).

### 23.5 Migración Incremental

Pasos 1–9 detallados.

### 23.6 Tests Estratificados

Tabla niveles (unidad dominio, core, adaptadores, persistencia, políticas, proveedores, integración, end-to-end, benchmarks, fuzzing).

### 23.7 Tooling / Scripts

Lista scripts (dev_db.sh, deploy_migrations.sh, lint_all.sh, test_all.sh, coverage.sh, check_deps.sh, regenerate_diagrams.sh).

### 23.8 Principios Aceptación

Lista 1–7 (cero ciclos, core neutro, etc.)

### 23.9 Próximos Pasos

deny.toml, publicar core, cargo-nextest, criterion, lint arquitectura.
