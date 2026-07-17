DROP FUNCTION IF EXISTS get_onboarding_status(integer);

CREATE OR REPLACE FUNCTION get_onboarding_status(p_doctor_account_id integer)
    RETURNS TABLE
            (
                status        doctor_profile_status_enum,
                status_reason text
            )
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
    END IF;

    RETURN QUERY
        SELECT d.status,
               (SELECT t.status_reason
                FROM doctor_profile_transaction t
                WHERE t.doctor_account_id = d.doctor_account_id
                  AND t.status = d.status
                ORDER BY t.created_at DESC
                LIMIT 1) AS status_reason
        FROM doctor_profile_draft d
        WHERE d.doctor_account_id = p_doctor_account_id;
END
$$;
