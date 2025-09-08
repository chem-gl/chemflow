//! chem-policies – F6: Políticas de selección básica
//!
//! Provee contratos y una implementación inicial (MaxScore) para elegir una
//! preferencia de propiedad de manera determinista y auditable.

use chem_core::hashing::{hash_str, to_canonical_json};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Candidato a selección de propiedad.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PropertyCandidate {
    pub molecule_inchikey: String,
    pub property_kind: String,
    /// Valor tipado como JSON (en esta v1 mantenemos neutralidad).
    pub value: serde_json::Value,
    pub units: Option<String>,
    pub provider: Option<String>,
    pub version: Option<String>,
    pub quality: Option<String>,
    /// Score opcional; si None, se asume 0.0 para efectos de orden total.
    pub score: Option<f64>,
}

impl PropertyCandidate {
    /// Clave estable (key lógica) para el evento: inchikey|prop:kind
    pub fn stable_key(&self) -> String {
        format!("{}|prop:{}", self.molecule_inchikey, self.property_kind)
    }
    /// Hash estable del valor, basado en JSON canónico.
    pub fn value_hash(&self) -> String {
        let cj = to_canonical_json(&self.value);
        hash_str(&cj)
    }
}

/// Parámetros de selección soportados en v1.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", content = "params")]
pub enum SelectionParams {
    MaxScore(MaxScoreParams),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MaxScoreParams {
    pub tie_break: TieRule,
}

impl Default for MaxScoreParams {
    fn default() -> Self {
        Self { tie_break: TieRule::ByKeyThenValueHash }
    }
}

/// Regla de desempate determinista.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TieRule {
    /// Orden total: inchikey asc, luego value_hash asc.
    ByKeyThenValueHash,
}

/// Decisión de selección.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SelectionDecision {
    pub selected_key: String,
    /// id estático de la política que tomó la decisión.
    pub policy_id: String,
    /// Hash canónico de parámetros de la política.
    pub params_hash: String,
    /// Rationale tipado (se puede serializar a JSON canónico para el evento).
    pub rationale: Rationale,
}

/// Explicación tipada de la decisión.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rationale {
    pub policy_id: String,
    pub params: SelectionParams,
    pub considered_n: usize,
    pub selected_key: String,
    pub ties: Vec<String>,
    pub tie_break_rule: TieRule,
}

impl Rationale {
    /// JSON canónico para persistencia/auditoría.
    pub fn to_canonical_json(&self) -> serde_json::Value {
        // Orden estable de claves garantizado por to_canonical_json al hashear;
        // devolvemos Value normal para consumo del engine/test.
        serde_json::to_value(self).expect("serialize rationale")
    }
}

/// Contrato de políticas de selección deterministas.
pub trait PropertySelectionPolicy {
    fn id(&self) -> &'static str;
    fn choose(&self, candidates: &[PropertyCandidate], params: &SelectionParams) -> SelectionDecision;
}

/// Política: seleccionar mayor score, con desempate estable.
pub struct MaxScorePolicy;

impl MaxScorePolicy {
    pub fn new() -> Self {
        Self
    }
}

impl PropertySelectionPolicy for MaxScorePolicy {
    fn id(&self) -> &'static str {
        "max_score"
    }

    fn choose(&self, candidates: &[PropertyCandidate], params: &SelectionParams) -> SelectionDecision {
        let ms_params = match params {
            SelectionParams::MaxScore(p) => p.clone(),
        };
        let mut sorted = candidates.to_vec();
        sorted.sort_by(|a, b| {
            let sa = a.score.unwrap_or(0.0);
            let sb = b.score.unwrap_or(0.0);
            // Primero score desc
            match sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal) {
                std::cmp::Ordering::Equal => match ms_params.tie_break {
                    TieRule::ByKeyThenValueHash => {
                        let ka = a.stable_key();
                        let kb = b.stable_key();
                        match ka.cmp(&kb) {
                            std::cmp::Ordering::Equal => a.value_hash().cmp(&b.value_hash()),
                            o => o,
                        }
                    }
                },
                o => o,
            }
        });

        let selected = sorted.first().cloned().expect("non-empty candidates");
        let selected_key = selected.stable_key();
        let ties: Vec<String> = sorted
            .iter()
            .filter(|c| (c.score.unwrap_or(0.0) - selected.score.unwrap_or(0.0)).abs() < f64::EPSILON)
            .map(|c| c.stable_key())
            .collect();

        let params_hash = params_hash(params);
        let rationale = Rationale { policy_id: self.id().into(),
                                    params: params.clone(),
                                    considered_n: candidates.len(),
                                    selected_key: selected_key.clone(),
                                    ties,
                                    tie_break_rule: ms_params.tie_break };
        SelectionDecision { selected_key,
                            policy_id: self.id().into(),
                            params_hash,
                            rationale }
    }
}

/// Hash canónico de parámetros.
pub fn params_hash(params: &SelectionParams) -> String {
    let v = serde_json::to_value(params).expect("params serialize");
    let cj = to_canonical_json(&v);
    hash_str(&cj)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(key: &str, prop: &str, score: f64) -> PropertyCandidate {
        PropertyCandidate { molecule_inchikey: key.into(),
                            property_kind: prop.into(),
                            value: json!({"v": 1, "schema_version": 1}),
                            units: None,
                            provider: Some("sim".into()),
                            version: Some("1".into()),
                            quality: None,
                            score: Some(score) }
    }

    #[test]
    fn deterministic_selection_and_tie_break() {
        let p = MaxScorePolicy::new();
        let params = SelectionParams::MaxScore(MaxScoreParams::default());
        let cands = vec![cand("A", "foo", 0.9), cand("B", "foo", 0.9), cand("C", "foo", 0.8)];
        let d1 = p.choose(&cands, &params);
        let d2 = p.choose(&cands, &params);
        assert_eq!(d1.selected_key, d2.selected_key);
        // Con tie break por key, A gana ante B con mismo score
        assert_eq!(d1.selected_key, "A|prop:foo");
        assert_eq!(d1.policy_id, "max_score");
        // params_hash estable
        assert!(!d1.params_hash.is_empty());
    }

    #[test]
    fn params_hash_changes_with_params() {
        let p = MaxScorePolicy::new();
        let cands = vec![cand("A", "foo", 0.5), cand("B", "foo", 0.6)];
        let p1 = SelectionParams::MaxScore(MaxScoreParams { tie_break: TieRule::ByKeyThenValueHash });
        let p2 = SelectionParams::MaxScore(MaxScoreParams { tie_break: TieRule::ByKeyThenValueHash });
        let d1 = p.choose(&cands, &p1);
        let d2 = p.choose(&cands, &p2);
        assert_eq!(d1.params_hash, d2.params_hash);
    }
}
