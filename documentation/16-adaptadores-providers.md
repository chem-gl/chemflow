## 16. Providers / Adaptadores y Desacoplamiento

DomainStepAdapter como Anti‑Corruption Layer: empaqueta entidades químicas en artifacts neutrales (kind, hash, payload).

Garantías:

- Core ignora semántica química.
- Hash determinista + JSON estable.
- Evolución dominio sin romper motor.
