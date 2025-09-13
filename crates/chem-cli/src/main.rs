//! chem-cli: Command Line Interface for ChemFlow
//!
//! This is a simple CLI binary that demonstrates the ChemFlow engine.
//! For more advanced usage, see the main binary in the root.

use chem_core::FlowEngine;
use chem_core::{typed_artifact, typed_step};

fn main() {
    println!("🚀 ChemFlow CLI");
    println!("===============");

    // Define a simple artifact and step for demonstration
    typed_artifact!(SimpleArtifact { value: String });

    typed_step! {
        source SimpleSource {
            id: "simple_source",
            output: SimpleArtifact,
            params: (),
            run(_self, _p) {
                SimpleArtifact { value: "Hello from CLI!".to_string(), schema_version: 1 }
            }
        }
    }

    // Create and run a simple flow
    let mut engine = FlowEngine::builder(
        chem_core::event::InMemoryEventStore::default(),
        chem_core::repo::InMemoryFlowRepository::new()
    )
    .first_step(SimpleSource::new())
    .build();

    match engine.run_to_completion() {
        Ok(flow_id) => {
            println!("✅ Flow completed successfully!");
            println!("   Flow ID: {}", flow_id);
        }
        Err(e) => {
            println!("❌ Error running flow: {:?}", e);
        }
    }
}
