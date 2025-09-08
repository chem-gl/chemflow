# chem-policies (F6)

Contratos y una política inicial (MaxScore) para selección determinista de propiedades.

- Contratos: `PropertySelectionPolicy`, `PropertyCandidate`, `SelectionParams`, `SelectionDecision`, `Rationale`.
- Política incluida: `MaxScorePolicy` con desempate estable.
- `params_hash`: derivado de JSON canónico de `SelectionParams`.
- Evento asociado en el engine: `PropertyPreferenceAssigned` (no altera estado, auditable) emitido antes de `StepFinished`.

Ejemplo breve:

```rust
use chem_policies::{MaxScorePolicy, PropertyCandidate, SelectionParams, MaxScoreParams};
let p = MaxScorePolicy::new();
let params = SelectionParams::MaxScore(MaxScoreParams::default());
let cands = vec![/* ... */];
let decision = p.choose(&cands, &params);
println!("selected={}, params_hash={}", decision.selected_key, decision.params_hash);
```

Notas:

- Evitar floats no deterministas; normalizar precisión si corresponde.
- El engine mezcla `params_hash` al fingerprint del step únicamente cuando existe el evento de política.
