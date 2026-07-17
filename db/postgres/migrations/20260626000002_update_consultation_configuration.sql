CREATE OR REPLACE FUNCTION update_consultation_configuration(
    p_doctor_account_id integer,
    p_channel_types channel_type_enum[],
    p_supported_languages language_code_enum[],
    p_duration_minutes integer,
    p_doctor_fee_amount numeric,
    p_action_by integer
)
    RETURNS boolean
    LANGUAGE plpgsql
AS
$$
DECLARE
    v_doctor_id uuid;
    v_previous_fee_amount numeric(10, 2);
BEGIN
    SELECT dp.doctor_id
    INTO v_doctor_id
    FROM doctor_profile dp
    WHERE dp.doctor_account_id = p_doctor_account_id;

    IF v_doctor_id IS NULL THEN
        RETURN false;
    END IF;

    SELECT c.doctor_fee_amount
    INTO v_previous_fee_amount
    FROM doctor_consultation_config c
    WHERE c.doctor_id = v_doctor_id;

    INSERT INTO doctor_consultation_config (
        doctor_id,
        channel_types,
        supported_languages,
        duration_minutes,
        doctor_fee_amount
    )
    VALUES (
        v_doctor_id,
        p_channel_types,
        p_supported_languages,
        p_duration_minutes,
        p_doctor_fee_amount
    )
    ON CONFLICT ON CONSTRAINT doctor_configuration_pkey DO UPDATE SET
        channel_types = EXCLUDED.channel_types,
        supported_languages = EXCLUDED.supported_languages,
        duration_minutes = EXCLUDED.duration_minutes,
        doctor_fee_amount = EXCLUDED.doctor_fee_amount,
        updated_at = now();

    IF p_doctor_fee_amount IS NOT NULL
        AND v_previous_fee_amount IS DISTINCT FROM p_doctor_fee_amount THEN
        INSERT INTO doctor_fee_transaction (
            doctor_id,
            doctor_fee_amount,
            previous_fee_amount,
            action_by
        )
        VALUES (
            v_doctor_id,
            p_doctor_fee_amount,
            v_previous_fee_amount,
            p_action_by
        );
    END IF;

    RETURN true;
END
$$;
