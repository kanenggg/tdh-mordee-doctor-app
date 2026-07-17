-- Modify "consultation_schedule" default to include the timezone key
ALTER TABLE "consultation_schedule" ALTER COLUMN "schedule_config" SET DEFAULT '{"specificDate":[],"daysOfWeek":{},"timezone":"Asia/Bangkok"}'::jsonb;
-- Backfill timezone for existing rows that predate the field
UPDATE "consultation_schedule"
SET "schedule_config" = jsonb_set("schedule_config", '{timezone}', '"Asia/Bangkok"', true)
WHERE NOT "schedule_config" ? 'timezone';
-- Modify "get_consultation_schedule_config" function default fallback
CREATE OR REPLACE FUNCTION "get_consultation_schedule_config" ("p_doctor_id" integer, "p_biz_unit_id" integer) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
    v_config jsonb;
BEGIN
    SELECT cs.schedule_config
    INTO v_config
    FROM consultation_schedule cs
    WHERE cs.doctor_id = p_doctor_id
      AND cs.biz_unit_id = p_biz_unit_id;

    RETURN COALESCE(v_config, '{"specificDate":[],"daysOfWeek":{},"timezone":"Asia/Bangkok"}'::jsonb);
END;
$$;
