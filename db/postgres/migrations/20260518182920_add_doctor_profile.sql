create type doctor_profile_status_enum as enum ('Draft','PendingApproval', 'Approved', 'Rejected', 'Deactivated');

CREATE TABLE IF NOT EXISTS doctor_profile_draft (
                                                    doctor_account_id           integer                    not null primary key,
                                                    doctor_profile_id           integer                    not null,
                                                    citizen_id                  varchar(13),
    profession                  jsonb                    default '[]'::jsonb,
    academic_position           jsonb                    default '[]'::jsonb,
    first_name                  jsonb                    default '[]'::jsonb,
    last_name                   jsonb                    default '[]'::jsonb,
    license_number              varchar(50),
    primary_medical_school      jsonb                    default '[]'::jsonb,
    specialty                   jsonb                    default '[]'::jsonb,
    special_interest            text[]                   default '{}'::text[],
    address_detail              text,
    sub_district                jsonb,
    district                    jsonb,
    province                    jsonb,
    postal_code                 integer,
    work_place                  jsonb                    default '[]'::jsonb,
    additional_workplace        jsonb                    default '[]'::jsonb,
    profile_image_url           varchar(500),
    id_card_image_url           varchar(500),
    book_bank_image_url         varchar(500),
    medical_license_image_url   varchar(500),
    education_license_image_url text[]                   default '{}'::text[],
    status                      doctor_profile_status_enum not null,
    created_at                  timestamp with time zone default now(),
    updated_at                  timestamp with time zone
                                              );
CREATE INDEX idx_doctor_profile_draft_doctor_account_id ON doctor_profile_draft (doctor_account_id);

CREATE TABLE IF NOT EXISTS doctor_profile (
                                              doctor_account_id           integer                                       not null primary key,
                                              doctor_profile_id           integer                                       not null,
                                              citizen_id                  varchar(13)                                   not null unique,
    profession                  jsonb                    default '[]'::jsonb  not null,
    academic_position           jsonb                    default '[]'::jsonb  not null,
    first_name                  jsonb                    default '[]'::jsonb  not null,
    last_name                   jsonb                    default '[]'::jsonb  not null,
    license_number              varchar(50)                                   not null,
    primary_medical_school      jsonb                    default '[]'::jsonb  not null,
    specialty                   jsonb                    default '[]'::jsonb  not null,
    special_interest            text[]                   default '{}'::text[] not null,
    address_detail              text                                          not null,
    sub_district                jsonb                                         not null,
    district                    jsonb                                         not null,
    province                    jsonb                                         not null,
    postal_code                 integer                                       not null,
    work_place                  jsonb                    default '[]'::jsonb  not null,
    additional_workplace        jsonb                    default '[]'::jsonb  not null,
    profile_image_url           varchar(500)                                  not null,
    id_card_image_url           varchar(500)                                  not null,
    book_bank_image_url         varchar(500)                                  not null,
    medical_license_image_url   varchar(500)                                  not null,
    education_license_image_url text[]                   default '{}'::text[] not null,
    is_active                   boolean                  default false        not null,
    created_at                  timestamp with time zone default now(),
    updated_at                  timestamp with time zone
                                              );

CREATE INDEX idx_doctor_profile_doctor_account_id ON doctor_profile(doctor_account_id);

CREATE TABLE IF NOT EXISTS doctor_profile_transaction (
                                                          transaction_id      integer                     generated always as identity primary key,
                                                          doctor_account_id   integer                     not null,
                                                          status              doctor_profile_status_enum  not null,
                                                          status_reason       text                        not null,
                                                          action_by           varchar(50),
    created_at          timestamp with time zone default now()
    );
CREATE INDEX idx_doctor_profile_transaction_doctor_account_id ON doctor_profile_transaction(doctor_account_id);

DROP FUNCTION IF EXISTS get_doctor_profile_draft(integer);

CREATE OR REPLACE FUNCTION get_doctor_profile_draft(p_doctor_account_id integer)
    returns TABLE(citizen_id character varying, profession jsonb, academic_position jsonb, first_name jsonb, last_name jsonb, license_number character varying, medical_school jsonb, specialty jsonb, special_interests text[], address_detail text, sub_district jsonb, district jsonb, province jsonb, postal_code integer, primary_workplace jsonb, additional_workplace jsonb, profile_image_url character varying, id_card_image_url character varying, book_bank_image_url character varying, medical_license_image_url character varying, education_license_image_url text[], status doctor_profile_status_enum)
    language plpgsql
as
$$
BEGIN
RETURN QUERY
SELECT o.citizen_id,
       o.profession,
       o.academic_position,
       o.first_name,
       o.last_name,
       o.license_number,
       o.primary_medical_school,
       o.specialty,
       o.special_interest,
       o.address_detail,
       o.sub_district,
       o.district,
       o.province,
       o.postal_code,
       o.work_place,
       o.additional_workplace,
       o.profile_image_url,
       o.id_card_image_url,
       o.book_bank_image_url,
       o.medical_license_image_url,
       o.education_license_image_url,
       o.status
FROM doctor_profile_draft o
WHERE o.doctor_account_id = p_doctor_account_id;
END;
$$;

DROP FUNCTION IF EXISTS save_doctor_profile_draft(integer, integer, varchar, jsonb, jsonb, jsonb, jsonb, varchar, jsonb, jsonb,
    text[], text, jsonb, jsonb, jsonb, integer, jsonb, jsonb, varchar, varchar,
    varchar, varchar, text[]);

CREATE OR REPLACE FUNCTION save_doctor_profile_draft(p_doctor_account_id integer, p_doctor_profile_id integer, p_citizen_id character varying, p_profession jsonb, p_academic_position jsonb, p_first_name jsonb, p_last_name jsonb, p_license_number character varying, p_medical_school jsonb, p_specialty jsonb, p_special_interests text[], p_address_detail text, p_sub_district jsonb, p_district jsonb, p_province jsonb, p_postal_code integer, p_primary_workplace jsonb, p_additional_workplace jsonb, p_profile_image_url character varying, p_id_card_image_url character varying, p_book_bank_image_url character varying, p_medical_license_image_url character varying, p_education_certificate_image_urls text[]) returns void
    language plpgsql
as
$$
BEGIN
insert into doctor_profile_draft (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
                                  first_name, last_name, license_number, primary_medical_school, specialty,
                                  special_interest,
                                  address_detail, sub_district, district, province, postal_code, work_place,
                                  additional_workplace, profile_image_url, id_card_image_url,
                                  book_bank_image_url, medical_license_image_url, education_license_image_url,status)
VALUES (p_doctor_account_id,
        p_doctor_profile_id,
        p_citizen_id,
        p_profession,
        p_academic_position,
        p_first_name,
        p_last_name,
        p_license_number,
        p_medical_school,
        p_specialty,
        p_special_interests,
        p_address_detail,
        p_sub_district,
        p_district,
        p_province,
        p_postal_code,
        p_primary_workplace,
        p_additional_workplace,
        p_profile_image_url,
        p_id_card_image_url,
        p_book_bank_image_url,
        p_medical_license_image_url,
        p_education_certificate_image_urls,
        'Draft'
       )
    ON CONFLICT (doctor_account_id)
        DO UPDATE SET citizen_id                  = EXCLUDED.citizen_id,
                   profession                  = EXCLUDED.profession,
                   academic_position           = EXCLUDED.academic_position,
                   first_name                  = EXCLUDED.first_name,
                   last_name                   = EXCLUDED.last_name,
                   license_number              = EXCLUDED.license_number,
                   primary_medical_school      = EXCLUDED.primary_medical_school,
                   specialty                   = EXCLUDED.specialty,
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
                   status                      = excluded.status,
                   updated_at                  = now();
-- insert transaction
INSERT INTO doctor_profile_transaction (doctor_account_id,status,status_reason,action_by)
VALUES (p_doctor_account_id,'Draft','',p_doctor_account_id);
END;
$$;

DROP FUNCTION IF EXISTS submit_doctor_profile_draft(integer, integer, varchar, jsonb, jsonb, jsonb, jsonb, varchar, jsonb, jsonb,
    text[], text, jsonb, jsonb, jsonb, integer, jsonb, jsonb, varchar, varchar,
    varchar, varchar, text[]);

CREATE OR REPLACE FUNCTION submit_doctor_profile_draft(p_doctor_account_id integer, p_doctor_profile_id integer, p_citizen_id character varying, p_profession jsonb, p_academic_position jsonb, p_first_name jsonb, p_last_name jsonb, p_license_number character varying, p_medical_school jsonb, p_specialty jsonb, p_special_interests text[], p_address_detail text, p_sub_district jsonb, p_district jsonb, p_province jsonb, p_postal_code integer, p_primary_workplace jsonb, p_additional_workplace jsonb, p_profile_image_url character varying, p_id_card_image_url character varying, p_book_bank_image_url character varying, p_medical_license_image_url character varying, p_education_certificate_image_urls text[]) returns void
    language plpgsql
as
$$
BEGIN
insert into doctor_profile_draft (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
                                  first_name, last_name, license_number, primary_medical_school, specialty,
                                  special_interest,
                                  address_detail, sub_district, district, province, postal_code, work_place,
                                  additional_workplace, profile_image_url, id_card_image_url,
                                  book_bank_image_url, medical_license_image_url, education_license_image_url,status)
VALUES (p_doctor_account_id,
        p_doctor_profile_id,
        p_citizen_id,
        p_profession,
        p_academic_position,
        p_first_name,
        p_last_name,
        p_license_number,
        p_medical_school,
        p_specialty,
        p_special_interests,
        p_address_detail,
        p_sub_district,
        p_district,
        p_province,
        p_postal_code,
        p_primary_workplace,
        p_additional_workplace,
        p_profile_image_url,
        p_id_card_image_url,
        p_book_bank_image_url,
        p_medical_license_image_url,
        p_education_certificate_image_urls,
        'PendingApproval'
       )
    ON CONFLICT (doctor_account_id)
        DO UPDATE SET citizen_id                  = EXCLUDED.citizen_id,
                   profession                  = EXCLUDED.profession,
                   academic_position           = EXCLUDED.academic_position,
                   first_name                  = EXCLUDED.first_name,
                   last_name                   = EXCLUDED.last_name,
                   license_number              = EXCLUDED.license_number,
                   primary_medical_school      = EXCLUDED.primary_medical_school,
                   specialty                   = EXCLUDED.specialty,
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
                   status                      = excluded.status,
                   updated_at                  = now();
-- insert transaction
INSERT INTO doctor_profile_transaction (doctor_account_id,status,status_reason,action_by)
VALUES (p_doctor_account_id,'PendingApproval','',p_doctor_account_id);
END;
$$;