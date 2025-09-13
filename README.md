# ChemFlow

Resumen
-------
ChemFlow es un motor determinista de workflows orientado a pipelines de pasos (steps). El proyecto está organizado en crates Rust separados: `chem-core` (motor y modelos), `chem-domain` (tipos del dominio químico), `chem-persistence` (persistencia Postgres/Diesel), `chem-adapters` (adapters dominio↔core) y utilidades.

Estado actual
------------
- `chem-core`: motor lineal determinista, event store trait y `InMemoryEventStore`.
- `chem-persistence`: implementación Postgres con Diesel (`PgEventStore`, `PgFlowRepository`) y migraciones.
- `chem-adapters`: pasos de ejemplo (Acquire/Compute) y encoders dominio→artifact.

Quick start
-----------
Prerequisitos:
- Rust toolchain (stable) y `cargo`.
- Para pruebas con Postgres: tener `DATABASE_URL` apuntando a una instancia Postgres (opcional para tests in-memory).

Ejecutar tests rápidos (sin Postgres):

```bash
cargo test -p chem-core
```

Ejecutar un test de integración que usa Postgres (si tienes DB):

```bash
export DATABASE_URL='postgres://admin:admin123@localhost:5432/mydatabase?gssencmode=disable'
RUST_TEST_THREADS=1 cargo test -p chem-persistence --test branch_and_recover -- --nocapture
```

Nota: En algunos entornos la combinación de `libpq` y `krb5` puede provocar abortos en el teardown de tests. Si eso ocurre, un workaround temporal en los tests de integración es usar `std::mem::forget(...)` para evitar ejecutar destructores nativos durante teardown. También se puede desactivar la negociación GSSAPI con `?gssencmode=disable` en la `DATABASE_URL`.

#sym:main — ejemplo de interacción humana
----------------------------------------
En el motor los eventos de interacción humana (por ejemplo `UserInteractionRequested` y `UserInteractionProvided`) se representan como variantes de `FlowEventKind` y se insertan en el `EventStore`. Un ejemplo de cómo se podría inyectar una acción (p.ej. respuesta de usuario) sería:

```rust
// Pseudocódigo de ejemplo (no compilable directamente aquí)
use chem_core::{EventStore, FlowEventKind};
use uuid::Uuid;
let mut store = /* un EventStore, p.ej. PgEventStore o InMemoryEventStore */;
let flow_id = Uuid::parse_str("...").unwrap();

// Cuando se recibe la acción del usuario, la representamos como evento
let user_action = FlowEventKind::UserInteractionProvided {
    interaction_id: "human_gate_1".to_string(),
    flow_id: flow_id,
    payload: serde_json::json!({"choice": "approve", "comment": "ok"}),
};

store.append_kind(flow_id, user_action);

// El engine o los componentes de replay leerán ese evento y continuarán
```

Cómo contribuir
---------------
- Ejecuta `cargo test --all` y corrige fallos.
- Mantén las migraciones en `crates/chem-persistence/migrations`.

Licencia y notas
-----------------
Proyecto educativo / experimental. Consulta los archivos `Cargo.toml` de cada crate para dependencias y versiones.

---
Archivo creado automáticamente por la herramienta de mantenimiento del repo.
