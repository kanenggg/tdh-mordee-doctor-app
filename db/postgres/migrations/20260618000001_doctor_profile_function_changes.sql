DROP FUNCTION IF EXISTS save_doctor_profile_draft(
    integer, integer, varchar, jsonb, jsonb, jsonb, jsonb, varchar, jsonb,
    jsonb, text[], text, jsonb, jsonb, jsonb, integer, jsonb, jsonb,
    varchar, varchar, varchar, varchar, text[]
);
DROP FUNCTION IF EXISTS submit_doctor_profile_draft(int4, int4, varchar, jsonb, jsonb, jsonb, jsonb, varchar, jsonb,
                                                    jsonb, text[], text, jsonb, jsonb, jsonb, int4, jsonb, jsonb,
                                                    varchar, varchar, varchar, varchar, text[]);


DROP FUNCTION IF EXISTS approve_doctor_profile_draft(uuid, integer, integer);

DROP FUNCTION IF EXISTS approve_doctor_profile_draft(integer, integer);
DROP FUNCTION IF EXISTS approve_doctor_profile_draft(integer, integer, text);

DROP FUNCTION IF EXISTS get_doctor_profile_draft(integer);
DROP FUNCTION IF EXISTS get_doctor_profile(integer);

CREATE OR REPLACE FUNCTION public.save_doctor_profile_draft(
    p_doctor_account_id integer,
    p_doctor_profile_id integer,
    p_citizen_id character varying,
    p_profession jsonb,
    p_academic_position jsonb,
    p_first_name jsonb,
    p_last_name jsonb,
    p_license_number character varying,
    p_medical_school jsonb,
    p_specialty jsonb,
    p_special_interests text[],
    p_address_detail text,
    p_sub_district jsonb,
    p_district jsonb,
    p_province jsonb,
    p_postal_code integer,
    p_primary_workplace jsonb,
    p_additional_workplace jsonb,
    p_profile_image_url character varying,
    p_id_card_image_url character varying,
    p_book_bank_image_url character varying,
    p_medical_license_image_url character varying,
    p_education_certificate_image_urls text[],
    p_additional_specialties jsonb DEFAULT '[]'::jsonb
)
    RETURNS void
    LANGUAGE plpgsql
AS
$$
BEGIN
    INSERT INTO doctor_profile_draft (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
                                      first_name, last_name, license_number, primary_medical_school, specialty,
                                      additional_specialties, special_interest, address_detail,
                                      sub_district, district, province, postal_code, work_place, additional_workplace,
                                      profile_image_url, id_card_image_url, book_bank_image_url,
                                      medical_license_image_url, education_license_image_url, status)
    VALUES (p_doctor_account_id, p_doctor_profile_id, p_citizen_id,
            p_profession, p_academic_position,
            p_first_name, p_last_name, p_license_number, p_medical_school, p_specialty,
            p_additional_specialties, p_special_interests, p_address_detail,
            p_sub_district, p_district, p_province, p_postal_code,
            p_primary_workplace, p_additional_workplace,
            p_profile_image_url, p_id_card_image_url, p_book_bank_image_url,
            p_medical_license_image_url, p_education_certificate_image_urls,
            'Draft')
    ON CONFLICT (doctor_account_id) DO UPDATE SET doctor_profile_id           = EXCLUDED.doctor_profile_id,
                                                  citizen_id                  = EXCLUDED.citizen_id,
                                                  profession                  = EXCLUDED.profession,
                                                  academic_position           = EXCLUDED.academic_position,
                                                  first_name                  = EXCLUDED.first_name,
                                                  last_name                   = EXCLUDED.last_name,
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
                                                  status                      = 'Draft',
                                                  updated_at                  = now();

    INSERT INTO doctor_profile_transaction (doctor_account_id, status, status_reason, action_by)
    VALUES (p_doctor_account_id, 'Draft', '', p_doctor_account_id);
END
$$;


CREATE OR REPLACE FUNCTION submit_doctor_profile_draft(
    p_doctor_account_id integer,
    p_doctor_profile_id integer,
    p_citizen_id varchar,
    p_profession jsonb,
    p_academic_position jsonb,
    p_first_name jsonb,
    p_last_name jsonb,
    p_license_number varchar,
    p_medical_school jsonb,
    p_specialty jsonb,
    p_special_interests text[],
    p_address_detail text,
    p_sub_district jsonb,
    p_district jsonb,
    p_province jsonb,
    p_postal_code integer,
    p_primary_workplace jsonb,
    p_additional_workplace jsonb,
    p_profile_image_url varchar,
    p_id_card_image_url varchar,
    p_book_bank_image_url varchar,
    p_medical_license_image_url varchar,
    p_education_certificate_image_urls text[],
    p_additional_specialties jsonb DEFAULT '[]'::jsonb
)
    RETURNS void
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
    END IF;

    INSERT INTO doctor_profile_draft (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
                                      first_name, last_name, license_number, primary_medical_school, specialty,
                                      additional_specialties, special_interest, address_detail,
                                      sub_district, district, province, postal_code, work_place, additional_workplace,
                                      profile_image_url, id_card_image_url, book_bank_image_url,
                                      medical_license_image_url, education_license_image_url,
                                      status, created_at, updated_at)
    VALUES (p_doctor_account_id, p_doctor_profile_id, p_citizen_id,
            p_profession,
            p_academic_position,
            p_first_name,
            p_last_name,
            p_license_number,
            p_medical_school,
            p_specialty,
            p_additional_specialties,
            p_special_interests,
            p_address_detail,
            p_sub_district, p_district, p_province, p_postal_code,
            p_primary_workplace,
            p_additional_workplace,
            p_profile_image_url, p_id_card_image_url, p_book_bank_image_url,
            p_medical_license_image_url,
            p_education_certificate_image_urls,
            'PendingApproval', now(), now())
    ON CONFLICT (doctor_account_id) DO UPDATE SET doctor_profile_id           = EXCLUDED.doctor_profile_id,
                                                  citizen_id                  = EXCLUDED.citizen_id,
                                                  profession                  = EXCLUDED.profession,
                                                  academic_position           = EXCLUDED.academic_position,
                                                  first_name                  = EXCLUDED.first_name,
                                                  last_name                   = EXCLUDED.last_name,
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
                                                  status                      = 'PendingApproval',
                                                  updated_at                  = now();

    INSERT INTO doctor_profile_transaction (doctor_account_id, status, status_reason, action_by)
    VALUES (p_doctor_account_id, 'PendingApproval', '', p_doctor_account_id);
END
$$;


CREATE OR REPLACE FUNCTION approve_doctor_profile_draft(
    p_doctor_account_id integer,
    p_action_by integer,
    p_department_id integer
)
    RETURNS void
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
    END IF;

    UPDATE doctor_profile_draft
    SET status     = 'Approved',
        updated_at = now()
    WHERE doctor_account_id = p_doctor_account_id
      AND status = 'PendingApproval';

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
    ON CONFLICT (doctor_account_id) DO UPDATE SET doctor_profile_id           = EXCLUDED.doctor_profile_id,
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
END
$$;

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
