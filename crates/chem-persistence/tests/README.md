Instrucciones para tests de integración de `chem-persistence`

Estos tests usan una base de datos Postgres real. Antes de ejecutarlos, exporta
la variable de entorno `DATABASE_URL` apuntando a un Postgres donde el usuario
tenga permisos para crear tablas y ejecutar migraciones.

Ejemplo (linux/mac):

```bash
export DATABASE_URL="postgres://admin:admin123@localhost:5432/mydatabase?gssencmode=disable"
cargo test -p chem-persistence --test branching_db -- --nocapture
cargo test -p chem-persistence --test branching_rehydrate -- --nocapture
cargo test -p chem-persistence --test branching_declarative -- --nocapture
```

Notas:

- Los tests detectan si `DATABASE_URL` no está presente y se saltan (no fallan).
- `build_pool` ejecuta las migraciones embebidas la primera vez que se conecta.
- Si ejecutas en CI, levanta un servicio Postgres (docker-compose está en `postgress-docker/compose.yaml`) y configura `DATABASE_URL` apropiadamente.
