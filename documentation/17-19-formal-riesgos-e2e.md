## 17. Definiciones Formales (Tablas Resumen)

Referencias a clases Core, jerarquía dominio y mapeo sincronizados.

## 18. Anti‑Patrones y Riesgos

| Riesgo                      | Descripción                  | Mitigación                          |
| --------------------------- | ---------------------------- | ----------------------------------- |
| Mutación de familia         | Cambiar miembros tras freeze | Enforce frozen + rechazar UPDATE    |
| Fingerprint no canónico     | Normalización inconsistente  | canonical_json único                |
| Reprocesar sin idempotencia | Duplicación artifacts        | Hash estable + UNIQUE(hash)         |
| Eventos fuera de orden      | Concurrencia                 | seq BIGSERIAL + transacción atómica |
| Mezcla semántica en Core    | Lógica química en motor      | Capa adapter + revisión             |

## 19. Ejemplo End‑To‑End (Resumen)

```mermaid
flowchart LR
  A[Acquire Molecules] --> B[Build Families] --> C[Compute Properties] --> D[Select Preferred (Policy)] --> E[Aggregate Metrics] --> F{Branch Criteria Met?}
  F -- Yes --> BR[Create Branch] --> C
  F -- No --> G{Human Gate?}
  G -- Yes --> UI[Await User Input] --> G
  G -- No --> H[Generate Report] --> I[Persist Artifacts / Publish]
```

Descripción: flujo lineal con branching determinista y gate humano; eventos y artifacts hashados aseguran reproducibilidad.
