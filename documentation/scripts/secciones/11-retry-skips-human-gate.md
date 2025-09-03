# Sección 11 - Retry / Skips / Human Gate

| Caso             | Transición                | Notas                                      |
| ---------------- | ------------------------- | ------------------------------------------ |
| Error validación | Pending→Failed            | No ejecuta run                             |
| Error runtime    | Running→Failed            | Conserva artifacts parciales descartados   |
| Retry manual     | Failed→Pending            | Nuevo StepStarted crea historial adicional |
| Skip política    | Pending→Skipped           | Emite StepSkipped                          |
| Gate humano      | Running→AwaitingUserInput | Suspende avance cursor                     |
| Input humano     | AwaitingUserInput→Running | Reanuda run (o finaliza)                   |

### 11.1 Extensión de Modelo para Reintentos

Se añaden campos a `WORKFLOW_STEP_EXECUTIONS`:

- `retry_count INT NOT NULL DEFAULT 0` – número de reintentos consumidos.
- `max_retries INT NULL` – política establecida para ese step (nullable si ilimitado/externo).

Nueva tabla `STEP_EXECUTION_ERRORS` para registrar cada fallo (incluye validaciones y runtime) sin sobre‑escribir datos previos:

| Campo          | Tipo        | Descripción                                 |
| -------------- | ----------- | ------------------------------------------- |
| error_id       | UUID PK     | Identidad del registro de error             |
| step_id        | UUID FK     | Referencia a WORKFLOW_STEP_EXECUTIONS       |
| attempt_number | INT         | 0 = intento original, >0 reintentos         |
| ts             | TIMESTAMPTZ | Timestamp del error                         |
| error_class    | TEXT        | Clasificación (Validation, Runtime, Policy) |
| error_code     | TEXT        | Código programático opcional                |
| message        | TEXT        | Mensaje corto                               |
| details        | JSONB       | Stack / payload estructurado                |
| transient      | BOOLEAN     | Sugerencia de elegibilidad a retry          |

Beneficios: auditoría precisa, métricas MTTR/MTBF y selección inteligente de políticas de retry.

