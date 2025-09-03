# Sección 9 - Fingerprint / Reproducibilidad (Profundizado)

Composición mínima: sorted(hashes inputs) + canonical_json(params_sin_runtime) + step_kind + internal_version + provider_matrix_sorted + schema_version + deterministic_flag (+ seed).  
Uso: (a) caching, (b) comparación de ramas, (c) auditoría divergencias, (d) invalidación selectiva.

