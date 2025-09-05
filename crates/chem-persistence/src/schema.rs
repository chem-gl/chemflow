//! Esquema Diesel (generado manualmente para F3). Reemplazable con `diesel print-schema`.

diesel::table! {
    event_log (seq) {
        seq -> BigInt,
        flow_id -> Uuid,
        ts -> Timestamptz,
        event_type -> Text,
        payload -> Jsonb,
    }
}

diesel::table! {
    workflow_step_artifacts (artifact_hash) {
        artifact_hash -> Text,
        kind -> Text,
        payload -> Jsonb,
        metadata -> Nullable<Jsonb>,
        produced_in_seq -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    event_log,
    workflow_step_artifacts,
);
