# chem-adapters (F4)

Adaptadores Dominio ↔ Core: artifacts tipados neutrales, encoder de dominio y steps iniciales (Acquire / Compute).

- artifacts.rs: FamilyArtifact, FamilyPropertiesArtifact, MoleculeArtifact, MolecularPropertyArtifact
- encoder.rs: DomainArtifactEncoder + SimpleDomainEncoder
- steps/: acquire.rs (Source), compute.rs (Transform)

Ver `examples/basic_workflow.toml` para un pipeline básico.
