-- 0004: Soporte de evento 'retryscheduled' en event_log.event_type
-- Extiende el CHECK nominal agregado en 0003.

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
            'flowcompleted'
        ));
