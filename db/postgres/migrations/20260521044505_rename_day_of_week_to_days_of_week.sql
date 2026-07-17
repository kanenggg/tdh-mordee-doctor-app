-- Modify "consultation_schedule" table
ALTER TABLE "consultation_schedule" ALTER COLUMN "schedule_config" SET DEFAULT '{"specificDate":[],"daysOfWeek":{}}'::jsonb;
-- Rename existing schedule_config JSON key from "dayOfWeek" to "daysOfWeek"
UPDATE "consultation_schedule"
SET "schedule_config" = jsonb_set(
    "schedule_config" - 'dayOfWeek',
    '{daysOfWeek}',
    "schedule_config" -> 'dayOfWeek',
    true
)
WHERE "schedule_config" ? 'dayOfWeek'
  AND NOT "schedule_config" ? 'daysOfWeek';
-- Drop old key when both keys are present and the new key should win
UPDATE "consultation_schedule"
SET "schedule_config" = "schedule_config" - 'dayOfWeek'
WHERE "schedule_config" ? 'dayOfWeek'
  AND "schedule_config" ? 'daysOfWeek';
-- Modify "get_consultation_schedule_config" function
CREATE OR REPLACE FUNCTION "get_consultation_schedule_config" ("p_doctor_id" integer, "p_biz_unit_id" integer) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
    v_config jsonb;
BEGIN
    SELECT cs.schedule_config
    INTO v_config
    FROM consultation_schedule cs
    WHERE cs.doctor_id = p_doctor_id
      AND cs.biz_unit_id = p_biz_unit_id;

    RETURN COALESCE(v_config, '{"specificDate":[],"daysOfWeek":{}}'::jsonb);
END;
$$;
