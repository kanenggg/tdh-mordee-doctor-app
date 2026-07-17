DROP FUNCTION IF EXISTS approve_doctor_profile_draft(
    integer,
    integer,
    integer,
    channel_type_enum[],
    language_code_enum[],
    integer,
    numeric,
    integer[]
);

CREATE OR REPLACE FUNCTION approve_doctor_profile_draft(
    p_doctor_account_id integer,
    p_action_by integer,
    p_department_id integer,
    p_channel_types channel_type_enum[],
    p_supported_languages language_code_enum[],
    p_duration_minutes integer,
    p_doctor_fee_amount numeric,
    p_clinic_ids integer[]
)
    RETURNS TABLE
            (
                doctor_id         uuid,
                doctor_account_id integer,
                doctor_profile_id integer,
                department_id     integer,
                is_active         boolean,
                profession        jsonb,
                academic_position jsonb,
                first_name        jsonb,
                last_name         jsonb,
                profile_image_url character varying,
                approved_at       bigint,
                newly_approved    boolean
            )
    LANGUAGE plpgsql
AS
$$
DECLARE
    v_was_pending boolean;
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
    END IF;

    SELECT EXISTS (SELECT 1
                   FROM doctor_profile_draft d
                   WHERE d.doctor_account_id = p_doctor_account_id
                     AND d.status = 'PendingApproval')
    INTO v_was_pending;

    IF v_was_pending THEN
        UPDATE doctor_profile_draft d
        SET status     = 'Approved',
            updated_at = now()
        WHERE d.doctor_account_id = p_doctor_account_id
          AND d.status = 'PendingApproval';

        INSERT INTO doctor_profile (doctor_id, doctor_account_id, doctor_profile_id, citizen_id,
                                    profession, academic_position, first_name, last_name,
                                    department_id, license_number, primary_medical_school, specialty,
                                    additional_specialties, special_interest,
                                    address_detail, sub_district, district, province, postal_code,
                                    work_place, additional_workplace,
                                    profile_image_url, id_card_image_url, book_bank_image_url,
                                    medical_license_image_url, education_license_image_url, is_active)
        SELECT uuid_generate_v7(),
               d.doctor_account_id,
               d.doctor_profile_id,
               d.citizen_id,
               d.profession,
               d.academic_position,
               d.first_name,
               d.last_name,
               p_department_id,
               d.license_number,
               d.primary_medical_school,
               d.specialty,
               d.additional_specialties,
               d.special_interest,
               d.address_detail,
               d.sub_district,
               d.district,
               d.province,
               d.postal_code,
               d.work_place,
               d.additional_workplace,
               d.profile_image_url,
               d.id_card_image_url,
               d.book_bank_image_url,
               d.medical_license_image_url,
               d.education_license_image_url,
               true
        FROM doctor_profile_draft d
        WHERE d.doctor_account_id = p_doctor_account_id
          AND d.status = 'Approved'
        ON CONFLICT ON CONSTRAINT doctor_profile_account_key DO UPDATE SET doctor_profile_id           = EXCLUDED.doctor_profile_id,
                                                                           citizen_id                  = EXCLUDED.citizen_id,
                                                                           profession                  = EXCLUDED.profession,
                                                                           academic_position           = EXCLUDED.academic_position,
                                                                           first_name                  = EXCLUDED.first_name,
                                                                           last_name                   = EXCLUDED.last_name,
                                                                           department_id               = EXCLUDED.department_id,
                                                                           license_number              = EXCLUDED.license_number,
                                                                           primary_medical_school      = EXCLUDED.primary_medical_school,
                                                                           specialty                   = EXCLUDED.specialty,
                                                                           additional_specialties      = EXCLUDED.additional_specialties,
                                                                           special_interest            = EXCLUDED.special_interest,
                                                                           address_detail              = EXCLUDED.address_detail,
                                                                           sub_district                = EXCLUDED.sub_district,
                                                                           district                    = EXCLUDED.district,
                                                                           province                    = EXCLUDED.province,
                                                                           postal_code                 = EXCLUDED.postal_code,
                                                                           work_place                  = EXCLUDED.work_place,
                                                                           additional_workplace        = EXCLUDED.additional_workplace,
                                                                           profile_image_url           = EXCLUDED.profile_image_url,
                                                                           id_card_image_url           = EXCLUDED.id_card_image_url,
                                                                           book_bank_image_url         = EXCLUDED.book_bank_image_url,
                                                                           medical_license_image_url   = EXCLUDED.medical_license_image_url,
                                                                           education_license_image_url = EXCLUDED.education_license_image_url,
                                                                           is_active                   = EXCLUDED.is_active,
                                                                           updated_at                  = now();

        INSERT INTO doctor_profile_transaction (doctor_account_id, status, status_reason, action_by)
        VALUES (p_doctor_account_id, 'Approved', '', p_action_by);


        INSERT INTO doctor_consultation_config (doctor_id,
                                                channel_types,
                                                supported_languages,
                                                duration_minutes,
                                                doctor_fee_amount)
        SELECT dp.doctor_id,
               p_channel_types,
               p_supported_languages,
               p_duration_minutes,
               p_doctor_fee_amount
        FROM doctor_profile dp
        WHERE dp.doctor_account_id = p_doctor_account_id
          AND dp.is_active = true
        ON CONFLICT ON CONSTRAINT doctor_configuration_pkey DO UPDATE SET channel_types       = EXCLUDED.channel_types,
                                                                          supported_languages = EXCLUDED.supported_languages,
                                                                          duration_minutes    = EXCLUDED.duration_minutes,
                                                                          doctor_fee_amount   = EXCLUDED.doctor_fee_amount,
                                                                          updated_at          = now();

        INSERT INTO public.doctor_fee_transaction (doctor_id, doctor_fee_amount, action_by)
        SELECT dp.doctor_id,
               p_doctor_fee_amount,
               p_action_by
        FROM doctor_profile dp
        WHERE dp.doctor_account_id = p_doctor_account_id
          AND dp.is_active = true;

        INSERT INTO doctor_clinic (doctor_id, clinic_id)
        SELECT dp.doctor_id, c.clinic_id
        FROM doctor_profile dp
                 CROSS JOIN unnest(COALESCE(p_clinic_ids, ARRAY []::integer[])) AS c(clinic_id)
        WHERE dp.doctor_account_id = p_doctor_account_id
          AND dp.is_active = true
        ON CONFLICT ON CONSTRAINT doctor_clinic_pkey DO UPDATE SET updated_at = now();

    END IF;

    RETURN QUERY
        SELECT dp.doctor_id,
               dp.doctor_account_id,
               dp.doctor_profile_id,
               dp.department_id,
               dp.is_active,
               dp.profession,
               dp.academic_position,
               dp.first_name,
               dp.last_name,
               dp.profile_image_url,
               extract(epoch FROM now())::bigint,
               v_was_pending
        FROM doctor_profile dp
        WHERE dp.doctor_account_id = p_doctor_account_id
          AND dp.is_active = true;
END
$$;
