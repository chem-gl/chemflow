//! Hash helpers – abstracción para permitir cambiar de algoritmo sin tocar resto del core.
//! TODO: Evaluar blake3 vs sha256 según necesidades.

use blake3::Hasher;

/// Hashea un string y devuelve hex.
pub fn hash_str(input: &str) -> String {
    let mut h = Hasher::new();
    h.update(input.as_bytes());
    h.finalize().to_hex().to_string()
}
