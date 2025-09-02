# ChemFlow (Rust)

Plataforma experimental para orquestar flujos (workflows) de generación y cálculo de propiedades de familias de moléculas con trazabilidad y branching.

## Objetivos

- Adquirir familias de moléculas mediante proveedores (`MoleculeProvider`).
- Calcular propiedades sobre familias completas (`PropertiesProvider`).
- Generar datos agregados opcionales (`DataProvider`).
- Mantener trazabilidad completa: parámetros efectivos, proveedor, versión, timestamps y linaje (root / parent / branch).
- Soportar branching: ejecutar caminos alternativos a partir de un step previo conservando un `root_execution_id` común.

## Conceptos Clave

| Concepto              | Descripción                                                                                                |
| --------------------- | ---------------------------------------------------------------------------------------------------------- |
| `MoleculeFamily`      | Conjunto lógico de moléculas + propiedades calculadas + metadatos de proveedor.                            |
| `WorkflowStep`        | Unidad ejecutable (adquisición, cálculo de propiedades, etc.).                                             |
| `StepExecutionInfo`   | Snapshot de una ejecución: parámetros, proveedores usados, timestamps y relaciones (root, parent, branch). |
| `root_execution_id`   | Identificador compartido por todas las ejecuciones de un mismo flujo y sus ramas.                          |
| `parent_step_id`      | Step inmediatamente anterior en la cadena lineal.                                                          |
| `branch_from_step_id` | Step desde el cual se originó la rama actual (si aplica).                                                  |

## Estructura Resumida

```text
src/
  workflow/        -> Steps, manager y ejecución
  providers/       -> Traits e implementaciones de proveedores
  data/            -> Modelos de datos y tipos
  database/        -> Repositorio de persistencia (in-memory + Postgres opcional)
  migrations/      -> Runner de migraciones (archivos SQL en /migrations raíz)
```

## Requisitos

- Rust (stable) ≥ 1.78
- (Opcional) PostgreSQL 15+ si quieres persistencia real.
- Docker + Docker Compose (para levantar Postgres rápidamente):

```bash
docker compose -f postgress-docker/compose.yaml up -d
```

## Configuración de Entorno

Crear un archivo `.env` (o exportar variables):

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/chemflow
DATABASE_MIN_CONNECTIONS=2
```

## Migraciones

Coloca los SQL de esquema en `migrations/` (ya existen ejemplos: 0001*\*, 0002*\_, 0003\_\_). Para aplicar migraciones, simplemente ejecuta el binario principal que las corre al inicio:

```bash
cargo run
```

## Ejecución de Ejemplo

El `main.rs`:

1. Carga configuración y aplica migraciones.
2. Registra proveedores de ejemplo (moléculas y propiedades).
3. Ejecuta un flujo de adquisición y cálculo de propiedades.
4. Crea ramas (branch) adicionales demostrativas.
   Ejecutar:

```bash
cargo run
```

## Branching y Trazabilidad

Al ejecutar un step tras `create_branch(step_id_origen)`:

- `root_execution_id` permanece igual (linaje común).
- `parent_step_id` apunta al step desde el cual se encadenó la llamada.
- `branch_from_step_id` se establece al origen de la bifurcación.
  Puedes reconstruir el linaje consultando:

```rust
let steps = repo.get_steps_by_root(root_id).await;
```

Luego ordenar o agrupar por `branch_from_step_id`.

## Tests

Tests unitarios y un test de flujo con branching:

```bash
cargo test -- --nocapture
```

El test `tests/branching_flow.rs` valida:

- Creación de flujo.
- Ejecución de step de adquisición.
- Dos ramas desde el mismo step origen con ejecuciones independientes de propiedades.
- Consistencia de `root_execution_id` y conteo de ejecuciones con branch.

## Extender

- Añadir nuevos proveedores implementando los traits en `providers/`.
- Crear nuevos tipos de steps implementando `WorkflowStep`.
- Implementar persistencia completa de steps (nombre/descr) en DB si se requiere auditoría más rica.

## Comandos Rápidos

```bash
# Formatear
cargo fmt
# Compilar y testear
cargo test
# Ejecutar ejemplo
cargo run
```

## Licencia

Experimental / interna (definir según necesidad).
