#!/usr/bin/env bash
# Safe script to update event_log.event_type CHECK to include all known event kinds
# Usage: DATABASE_URL=... ./scripts/fix_event_type_check.sh
set -euo pipefail

if [ -z "${DATABASE_URL:-}" ]; then
  echo "Please set DATABASE_URL environment variable (e.g. export DATABASE_URL=postgres://user:pass@host:5432/db)"
  exit 1
fi

SQL_FILE=$(mktemp)
cat > "$SQL_FILE" <<'SQL'
-- Drop existing event_type CHECK constraints that mention event_type IN (...) then add canonical named constraint
DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN
        SELECT conname, pg_get_constraintdef(oid) AS def
        FROM pg_constraint
        WHERE conrelid = 'event_log'::regclass
          AND contype = 'c'
    LOOP
        IF r.def ILIKE '%event_type%' AND r.def ILIKE '% IN (%' THEN
            RAISE NOTICE 'Dropping constraint %', r.conname;
            EXECUTE format('ALTER TABLE event_log DROP CONSTRAINT %I', r.conname);
        END IF;
    END LOOP;
END$$;

ALTER TABLE event_log
    DROP CONSTRAINT IF EXISTS event_log_event_type_in_check;

ALTER TABLE event_log
    ADD CONSTRAINT event_log_event_type_in_check
        CHECK (event_type IN (
            'flowinitialized',
            'stepstarted',
            'stepfinished',
            'stepfailed',
            'stepsignal',
            'propertypreferenceassigned',
            'retryscheduled',
            'branchcreated',
            'userinteractionrequested',
            'userinteractionprovided',
            'flowcompleted'
        ));

SQL

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$SQL_FILE"
rm -f "$SQL_FILE"

echo "Event type CHECK updated successfully."
