//! Demo step that emits a reserved PROPERTY_PREFERENCE_ASSIGNED signal
//! and passes the input through. Useful to validate F6 end-to-end.

use chem_core::step::{StepKind, StepRunResultTyped, StepSignal, TypedStep};
use chem_core::typed_artifact;
use serde::{Deserialize, Serialize};
use serde_json::json;

typed_artifact!(DummyIn { v: i32 });
typed_artifact!(DummyOut { v: i32 });

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PolicyDemoParams;

#[derive(Clone, Debug)]
pub struct PolicyDemoStep;
impl PolicyDemoStep {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PolicyDemoStep {
    fn default() -> Self {
        Self::new()
    }
}

impl TypedStep for PolicyDemoStep {
    type Params = PolicyDemoParams;
    type Input = DummyIn;
    type Output = DummyOut;

    fn id(&self) -> &'static str {
        "policy_demo"
    }
    fn kind(&self) -> StepKind {
        StepKind::Transform
    }

    fn run_typed(&self, input: Option<Self::Input>, _params: Self::Params) -> StepRunResultTyped<Self::Output> {
        let inp = input.expect("policy_demo requires input");
        let data = json!({
            "property_key": "inchikey:DEMO|prop:foo",
            "policy_id": "max_score",
            "params_hash": "deadbeef",
            "rationale": {"demo": true}
        });
        let out = DummyOut { v: inp.v,
                             schema_version: 1 };
        StepRunResultTyped::SuccessWithSignals { outputs: vec![out],
                                                 signals: vec![StepSignal { signal:
                                                                                "PROPERTY_PREFERENCE_ASSIGNED".into(),
                                                                            data }] }
    }
}
