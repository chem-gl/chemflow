# Sección 18 - Anti‑Patrones y Riesgos

| Riesgo                      | Descripción                         | Mitigación                                  |
| --------------------------- | ----------------------------------- | ------------------------------------------- |
| Mutación de familia         | Cambiar miembros tras freeze        | Enforce frozen + rechazar UPDATE            |
| Fingerprint no canónico     | Orden o normalización inconsistente | Función canonical_json única                |
| Reprocesar sin idempotencia | Duplicación artifacts               | Hash estable previo a insert + UNIQUE(hash) |
| Eventos fuera de orden      | Concurrencia / race                 | sec BIGSERIAL + transacción atómica         |
| Mezcla semántica en Core    | Lógica química infiltrada           | Revisiones + capa adapter formal            |

