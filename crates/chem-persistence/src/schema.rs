//! Esquema Diesel (generado manualmente para F3). Reemplazable con `diesel
//! print-schema`.
//!
//! Tablas:
//! - `event_log`: log append-only de eventos por `flow_id` con `seq` como PK.
//! - `workflow_step_artifacts`: deduplicación por hash de artifacts producidos.

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

diesel::table! {
    step_execution_errors (id) {
        id -> BigInt,
        flow_id -> Uuid,
        step_id -> Text,
        attempt_number -> Integer,
        error_class -> Text,
        details -> Nullable<Jsonb>,
        ts -> Timestamptz,
    }
}

diesel::allow_tables_to_appear_in_same_query!(event_log, workflow_step_artifacts, step_execution_errors,);
