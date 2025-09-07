//! Insumos para el fingerprint de un Step.
//!
//! Este modelo define el shape de datos que se canonicaliza y hashea para
//! obtener un fingerprint determinista de la ejecución de un step, dependiente
//! de: versión del engine, id del step, hashes de input, parámetros y hash de
//! la definición del flow.
use serde::Serialize;
use serde_json::Value;

/// Estructura que agrupa los insumos para calcular fingerprint de un step.
/// NO es el fingerprint final (string hash) sino el modelo previo a
/// canonicalizar.
#[derive(Serialize)]
pub struct StepFingerprintInput<'a> {
    pub engine_version: &'a str,
    pub step_id: &'a str,
    pub input_hashes: &'a [String], // ordenadas lexicográficamente antes de crear esta estructura
    pub params: &'a Value,          // canonicalizable
    pub definition_hash: &'a str,
}
