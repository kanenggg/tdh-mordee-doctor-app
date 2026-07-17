CREATE OR REPLACE FUNCTION get_consultation_schedule_config(
    p_doctor_id integer,
    p_biz_unit_id integer
) RETURNS jsonb AS $$
DECLARE
    v_config jsonb;
BEGIN
    SELECT cs.schedule_config
    INTO v_config
    FROM consultation_schedule cs
    WHERE cs.doctor_id = p_doctor_id
      AND cs.biz_unit_id = p_biz_unit_id;

    RETURN COALESCE(v_config, '{"specificDate":[],"dayOfWeek":{}}'::jsonb);
END;
$$ LANGUAGE plpgsql STABLE;
