CREATE TABLE IF NOT EXISTS consultation_schedule (
    doctor_id integer NOT NULL,
    biz_unit_id integer NOT NULL,
    is_available boolean NOT NULL DEFAULT FALSE,
    schedule_config jsonb NOT NULL DEFAULT '{"specificDate":[],"daysOfWeek":{},"timezone":"Asia/Bangkok"}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (doctor_id, biz_unit_id)
);

CREATE INDEX IF NOT EXISTS idx_consultation_schedule_doctor_id ON consultation_schedule (doctor_id);
CREATE INDEX IF NOT EXISTS idx_consultation_instant_available_doctor
    ON consultation_instant (doctor_id)
    WHERE is_available = true;

CREATE INDEX IF NOT EXISTS idx_consultation_schedule_available_doctor
    ON consultation_schedule (doctor_id)
    WHERE is_available = true;

CREATE TABLE IF NOT EXISTS consultation_instant (
    doctor_id integer NOT NULL,
    is_available boolean NOT NULL DEFAULT FALSE,
    biz_unit_id integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (doctor_id, biz_unit_id)
);

CREATE INDEX IF NOT EXISTS idx_consultation_instant_doctor_id ON consultation_instant (doctor_id);

CREATE OR REPLACE FUNCTION save_consultation_schedule_config(
    p_doctor_id integer,
    p_biz_unit_id integer,
    p_schedule_config jsonb
) RETURNS void AS $$
BEGIN
    INSERT INTO consultation_schedule (
        doctor_id,
        biz_unit_id,
        schedule_config,
        updated_at
    )
    VALUES (
        p_doctor_id,
        p_biz_unit_id,
        p_schedule_config,
        now()
    )
    ON CONFLICT (doctor_id, biz_unit_id)
    DO UPDATE SET
        schedule_config = EXCLUDED.schedule_config,
        updated_at = now();
END;
$$ LANGUAGE plpgsql;

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

    RETURN COALESCE(v_config, '{"specificDate":[],"daysOfWeek":{},"timezone":"Asia/Bangkok"}'::jsonb);
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION set_consultation_schedule_availability(
    p_doctor_id integer,
    p_biz_unit_id integer,
    p_available boolean
) RETURNS void AS $$
BEGIN
    INSERT INTO consultation_schedule (
        doctor_id,
        biz_unit_id,
        is_available,
        updated_at
    )
    VALUES (
        p_doctor_id,
        p_biz_unit_id,
        p_available,
        now()
    )
    ON CONFLICT (doctor_id, biz_unit_id)
    DO UPDATE SET
        is_available = EXCLUDED.is_available,
        updated_at = now();
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION set_consultation_instant_availability(
    p_doctor_id integer,
    p_biz_unit_id integer,
    p_available boolean
) RETURNS void AS $$
BEGIN
    INSERT INTO consultation_instant (
        doctor_id,
        biz_unit_id,
        is_available,
        updated_at
    )
    VALUES (
        p_doctor_id,
        p_biz_unit_id,
        p_available,
        now()
    )
    ON CONFLICT (doctor_id, biz_unit_id)
    DO UPDATE SET
        is_available = EXCLUDED.is_available,
        updated_at = now();
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION get_consultation_availability(
    p_doctor_id integer,
    p_biz_unit_id integer
) RETURNS TABLE (
    schedule_available boolean,
    instant_available boolean
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        COALESCE(
            (
                SELECT cs.is_available
                FROM consultation_schedule cs
                WHERE cs.doctor_id = p_doctor_id
                  AND cs.biz_unit_id = p_biz_unit_id
            ),
            FALSE
        ) AS schedule_available,
        COALESCE(
            (
                SELECT ci.is_available
                FROM consultation_instant ci
                WHERE ci.doctor_id = p_doctor_id
                  AND ci.biz_unit_id = p_biz_unit_id
            ),
            FALSE
        ) AS instant_available;
END;
$$ LANGUAGE plpgsql STABLE;
