-- 0003: Extiende el conjunto permitido de event_type para incluir
-- 'propertypreferenceassigned' (F6).
--
-- Nota: En 0001 se definieron dos CHECKs de columna (lower() e IN (...)).
-- Aquí detectamos y eliminamos el CHECK del IN por introspección de catálogo
-- y añadimos una versión nueva y nominal para futuras migraciones.

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
            EXECUTE format('ALTER TABLE event_log DROP CONSTRAINT %I', r.conname);
        END IF;
    END LOOP;
END$$;

-- Añadimos un CHECK nominal (nombre estable) con el set actualizado
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
