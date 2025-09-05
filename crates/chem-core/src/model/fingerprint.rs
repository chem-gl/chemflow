use serde::Serialize;
use serde_json::Value;

/// Estructura que agrupa los insumos para calcular fingerprint de un step.
/// NO es el fingerprint final (string hash) sino el modelo previo a canonicalizar.
#[derive(Serialize)]
pub struct StepFingerprintInput<'a> {
    pub engine_version: &'a str,
    pub step_id: &'a str,
    pub input_hashes: &'a [String], // ordenadas lexicográficamente antes de crear esta estructura
    pub params: &'a Value,          // canonicalizable
    pub definition_hash: &'a str,
}
