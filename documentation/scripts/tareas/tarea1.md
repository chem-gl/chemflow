Molecule, MoleculeFamily, MolecularProperty, agregados numéricos, invariantes Hash familia + value_hash reproducibles 3 ejecuciones → mismos hashes Catálogo ampliado de futuras propiedades
Objetivos Clave:

Garantizar identidad y hash determinista.
Asegurar insert-only para propiedades.
Pasos sugeridos:

Molecule::new normaliza InChIKey.
MoleculeFamily::from_iter fija orden y calcula family_hash.
Test reproducibilidad (familia idéntica → mismo hash).
MolecularProperty::new genera value_hash.
Simular índice de unicidad de inchikey (estructura en tests).
Documentar invariantes (/// INVx:).
Revisión API pública y congelación.
GATE_F1:

Tests hash determinista pasan.
No hay mutadores post-freeze.
value_hash estable (snapshot test).
