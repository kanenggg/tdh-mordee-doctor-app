ALTER TABLE doctor_consultation_config
    DROP CONSTRAINT IF EXISTS doctor_duration_minutes_check;

ALTER TABLE doctor_consultation_config
    ADD CONSTRAINT doctor_duration_minutes_check
        CHECK (duration_minutes IS NULL OR duration_minutes = ANY (ARRAY [15, 25, 50]));