mod test_support;
use diesel::prelude::*;
use test_support::with_pool;
use uuid::Uuid;

// Test manual que intenta violar el constraint de event_type con un INSERT
// directo.
#[test]
fn event_type_constraint_rejects_invalid() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skip (no DATABASE_URL)");
        return;
    }
    let pool = with_pool(|p| p.clone()).unwrap();
    let mut conn = pool.get().unwrap();
    // Intento insertar tipo inválido
    let res = diesel::sql_query("INSERT INTO event_log (flow_id, event_type, payload) VALUES ($1, 'INVALID_TYPE', '{}'::jsonb)")
        .bind::<diesel::sql_types::Uuid, _>(Uuid::new_v4())
        .execute(&mut conn);
    assert!(res.is_err(), "Debe fallar constraint event_type");
}
