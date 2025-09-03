# ChemFlow (Rust)

Plataforma experimental para orquestar flujos (workflows) de generación y cálculo de propiedades de familias de moléculas con trazabilidad y branching.

## Objetivos

- Adquirir familias de moléculas mediante proveedores (`MoleculeProvider`).
- Calcular propiedades sobre familias completas (`PropertiesProvider`).
- Generar datos agregados opcionales (`DataProvider`).
- Mantener trazabilidad completa: parámetros efectivos, proveedor, versión, timestamps y linaje (root / parent / branch).
- Soportar branching: ejecutar caminos alternativos a partir de un step previo conservando un `root_execution_id` común.

## Conceptos Clave

TODO implementar conceptos clave aquí.

## Estructura Resumida

## Requisitos

- Rust (stable) ≥ 1.78
- (Opcional) PostgreSQL 15+ si quieres persistencia real.
- Docker + Docker Compose (para levantar Postgres rápidamente):

```bash
docker compose -f postgress-docker/compose.yaml up -d
```

## Configuración de Entorno

Crear un archivo `.env` (o exportar variables):

## Migraciones

## Ejecución de Ejemplo

...

## Comandos Rápidos

### Comandos Rápidos CLI

```bash
# Formatear
cargo fmt
# Compilar y testear
cargo test
# Ejecutar ejemplo
cargo run
```

## Contribuyendo

Lee [CONTRIBUTING.md](CONTRIBUTING.md) para detalles sobre cómo contribuir al proyecto.

## Licencia

Este proyecto está bajo la Licencia MIT. Consulta el archivo LICENSE para más detalles.
