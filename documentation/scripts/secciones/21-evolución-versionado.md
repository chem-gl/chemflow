# Sección 21 - Evolución / Versionado

Estrategia: versionado lógico en `internal_version` StepDefinition + `schema_version` global. Migraciones: añadir campos → tolerancia forward; remover requiere migración de proyecciones, nunca edición histórica de EVENT_LOG.

