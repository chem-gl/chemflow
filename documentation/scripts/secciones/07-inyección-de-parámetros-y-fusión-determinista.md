# Sección 7 - Inyección de Parámetros y Fusión Determinista

Orden estable (afecta reproducibilidad):

```text
merged = canonical_merge(
    base_params,
    injector_chain(flow,i),
    user_overrides?,
    human_gate_payload?,
    // runtime_derived  (NO entra en fingerprint)
)
```

Reglas de merge: última clave gana, arrays reemplazan (no concatenan) salvo que se marque `merge_strategy="append"` en metadata.

