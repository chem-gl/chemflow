-- Revertir 'retryscheduled' del CHECK nominal
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
            'flowcompleted'
        ));
