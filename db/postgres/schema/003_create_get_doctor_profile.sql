DROP FUNCTION IF EXISTS get_doctor_profile(integer);

CREATE OR REPLACE FUNCTION get_doctor_profile(p_doctor_account_id integer)
    RETURNS TABLE
            (
                doctor_id                   uuid,
                citizen_id                  text,  
                profession                  jsonb,
                academic_position           jsonb,
                first_name                  jsonb,
                last_name                   jsonb,
                license_number              varchar,
                medical_school              jsonb,
                specialty                   jsonb,
                additional_specialties      jsonb,
                department_id               integer,
                special_interests           text[],
                address_detail              text,
                sub_district                jsonb,
                district                    jsonb,
                province                    jsonb,
                postal_code                 integer,
                primary_workplace           jsonb,
                additional_workplace        jsonb,
                profile_image_url           varchar,
                id_card_image_url           varchar,
                book_bank_image_url         varchar,
                medical_license_image_url   varchar,
                education_license_image_url text[]
            )
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
    END IF;

    RETURN QUERY
        SELECT d.doctor_id,
               d.citizen_id,
               d.profession,
               d.academic_position,
               d.first_name,
               d.last_name,
               d.license_number,
               d.primary_medical_school,
               d.specialty,
               d.additional_specialties,
               d.department_id,
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
               d.education_license_image_url
        FROM doctor_profile d
        WHERE d.doctor_account_id = p_doctor_account_id
          AND d.is_active = true;
END
$$;

DROP FUNCTION IF EXISTS get_doctor_profile_draft(integer);

CREATE OR REPLACE FUNCTION get_doctor_profile_draft(p_doctor_account_id integer)
    RETURNS TABLE
            (
                citizen_id                  text,
                profession                  jsonb,
                academic_position           jsonb,
                first_name                  jsonb,
                last_name                   jsonb,
                license_number              varchar,
                medical_school              jsonb,
                specialty                   jsonb,
                additional_specialties      jsonb,
                special_interests           text[],
                address_detail              text,
                sub_district                jsonb,
                district                    jsonb,
                province                    jsonb,
                postal_code                 integer,
                primary_workplace           jsonb,
                additional_workplace        jsonb,
                profile_image_url           varchar,
                id_card_image_url           varchar,
                book_bank_image_url         varchar,
                medical_license_image_url   varchar,
                education_license_image_url text[],
                status                      doctor_profile_status_enum
            )
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
    END IF;

    RETURN QUERY
        SELECT d.citizen_id,
               d.profession,
               d.academic_position,
               d.first_name,
               d.last_name,
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
               d.status
        FROM doctor_profile_draft d
        WHERE d.doctor_account_id = p_doctor_account_id;
END
$$;
