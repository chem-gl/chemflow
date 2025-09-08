-- Revierte el cambio del conjunto permitido (elimina la variante F6) y
-- repone la lista original.

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
            'flowcompleted'
        ));
