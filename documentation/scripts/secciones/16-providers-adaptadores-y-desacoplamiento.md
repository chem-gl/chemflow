# Sección 16 - Providers / Adaptadores y Desacoplamiento

`DomainStepAdapter` aplica patrón Anti‑Corruption Layer: recibe outputs domain puros, encapsula en Artifact(kind, hash, payload) sin filtrar semántica. Cambios en estructuras químicas no impactan Core mientras se preserven: (a) determinismo de hash, (b) shape JSON estable.

