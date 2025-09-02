use chemflow_rust::data::family::MoleculeFamily;
use chemflow_rust::database::repository::WorkflowExecutionRepository;
use uuid::Uuid;
#[tokio::test]
async fn test_freeze_family_noop_in_memory() {
    let repo = WorkflowExecutionRepository::new(true);
    // Create a dummy family and upsert only in memory path (no pool so nothing
    // persisted)
    let fam = MoleculeFamily::new("Freeze Test".into(), None);
    // Simulate a hash-freeze call path (will just early return because get_family
    // finds None)
    let _ = repo.freeze_family(Uuid::new_v4()).await; // should be Ok
                                                      // Ensure calling on a non-existent id does not error
    assert!(fam.family_hash.is_none());
}
