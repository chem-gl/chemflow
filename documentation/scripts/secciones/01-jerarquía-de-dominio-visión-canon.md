# Sección 1 - Jerarquía de Dominio (Visión Canon)

Orden lógico y de dependencia (no ciclos):

1. Molecule (átomo de identidad química estable)
2. MoleculeFamily (colección ordenada congelada de moléculas)
3. Molecular Property Value (propiedad puntual por molécula)
4. Family Property (vista / agrupación lógica multi‑proveedor de valores de moléculas – opcional proyección)
5. Family Aggregate (estadístico derivado sobre familia)
6. Domain Artifact (cualquier empaquetado listo para Core)
7. Workflow Step Execution (metadatos de proceso)
8. Event (registro inmutable)

Cada nivel sólo referencia hashes/IDs del inmediatamente inferior → favorece desacoplamiento y caching.

