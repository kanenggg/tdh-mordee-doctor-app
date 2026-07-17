-- Onboarding database functions
-- These functions encapsulate the SQL logic for onboarding operations

-- Get onboarding by doctor account ID
CREATE OR REPLACE FUNCTION get_onboarding(p_doctor_account_id integer)
RETURNS TABLE (
    doctor_account_id integer,
    citizen_id varchar,
    profession_id integer,
    academic_position_id integer,
    license_number varchar,
    medical_school varchar,
    status onboarding_status_enum,
    status_reason text,
    address_detail text,
    sub_district_id integer,
    district_id integer,
    province_id integer,
    postal_code_id integer,
    profile_image_url varchar,
    id_card_image_url varchar,
    book_bank_image_url varchar,
    med_license_image_url varchar,
    certificate_image_urls text [],
    special_interests text [],
    name_en_firstname varchar,
    name_en_lastname varchar,
    name_th_firstname varchar,
    name_th_lastname varchar,
    primary_workplace_ids integer [],
    additional_workplace_ids integer [],
    specialties jsonb
) AS $$
BEGIN
  RETURN QUERY
  SELECT
    o.doctor_account_id,
    o.citizen_id,
    o.profession_id,
    o.academic_position_id,
    o.license_number,
    o.medical_school,
    o.status,
    o.status_reason,
    o.address_detail,
    o.sub_district_id,
    o.district_id,
    o.province_id,
    o.postal_code_id,
    o.profile_image_url,
    o.id_card_image_url,
    o.book_bank_image_url,
    o.med_license_image_url,
    o.certificate_image_urls,
    o.special_interests,
    o.name_en_firstname,
    o.name_en_lastname,
    o.name_th_firstname,
    o.name_th_lastname,
    o.primary_workplace_ids,
    o.additional_workplace_ids,
    o.specialties
  FROM onboarding o
  WHERE o.doctor_account_id = p_doctor_account_id;
END;
$$ LANGUAGE plpgsql;

-- Save or update onboarding information
CREATE OR REPLACE FUNCTION save_onboarding(
    p_doctor_account_id integer,
    p_citizen_id varchar,
    p_profession_id integer,
    p_academic_position_id integer,
    p_license_number varchar,
    p_medical_school varchar,
    p_status onboarding_status_enum,
    p_status_reason text,
    p_address_detail text,
    p_sub_district_id integer,
    p_district_id integer,
    p_province_id integer,
    p_postal_code_id integer,
    p_profile_image_url varchar,
    p_id_card_image_url varchar,
    p_book_bank_image_url varchar,
    p_med_license_image_url varchar,
    p_certificate_image_urls text [],
    p_special_interests text [],
    p_name_en_firstname varchar,
    p_name_en_lastname varchar,
    p_name_th_firstname varchar,
    p_name_th_lastname varchar,
    p_primary_workplace_ids integer [],
    p_additional_workplace_ids integer [],
    p_specialties jsonb
) RETURNS void AS $$
BEGIN
  INSERT INTO onboarding (
    doctor_account_id,
    citizen_id,
    profession_id,
    academic_position_id,
    license_number,
    medical_school,
    status,
    status_reason,
    address_detail,
    sub_district_id,
    district_id,
    province_id,
    postal_code_id,
    profile_image_url,
    id_card_image_url,
    book_bank_image_url,
    med_license_image_url,
    certificate_image_urls,
    special_interests,
    name_en_firstname,
    name_en_lastname,
    name_th_firstname,
    name_th_lastname,
    primary_workplace_ids,
    additional_workplace_ids,
    specialties
  ) VALUES (
    p_doctor_account_id,
    p_citizen_id,
    p_profession_id,
    p_academic_position_id,
    p_license_number,
    p_medical_school,
    p_status,
    p_status_reason,
    p_address_detail,
    p_sub_district_id,
    p_district_id,
    p_province_id,
    p_postal_code_id,
    p_profile_image_url,
    p_id_card_image_url,
    p_book_bank_image_url,
    p_med_license_image_url,
    p_certificate_image_urls,
    p_special_interests,
    p_name_en_firstname,
    p_name_en_lastname,
    p_name_th_firstname,
    p_name_th_lastname,
    p_primary_workplace_ids,
    p_additional_workplace_ids,
    p_specialties
  )
  ON CONFLICT (doctor_account_id)
  DO UPDATE SET
    citizen_id = EXCLUDED.citizen_id,
    profession_id = EXCLUDED.profession_id,
    academic_position_id = EXCLUDED.academic_position_id,
    license_number = EXCLUDED.license_number,
    medical_school = EXCLUDED.medical_school,
    status = EXCLUDED.status,
    status_reason = EXCLUDED.status_reason,
    address_detail = EXCLUDED.address_detail,
    sub_district_id = EXCLUDED.sub_district_id,
    district_id = EXCLUDED.district_id,
    province_id = EXCLUDED.province_id,
    postal_code_id = EXCLUDED.postal_code_id,
    profile_image_url = EXCLUDED.profile_image_url,
    id_card_image_url = EXCLUDED.id_card_image_url,
    book_bank_image_url = EXCLUDED.book_bank_image_url,
    med_license_image_url = EXCLUDED.med_license_image_url,
    certificate_image_urls = EXCLUDED.certificate_image_urls,
    special_interests = EXCLUDED.special_interests,
    name_en_firstname = EXCLUDED.name_en_firstname,
    name_en_lastname = EXCLUDED.name_en_lastname,
    name_th_firstname = EXCLUDED.name_th_firstname,
    name_th_lastname = EXCLUDED.name_th_lastname,
    primary_workplace_ids = EXCLUDED.primary_workplace_ids,
    additional_workplace_ids = EXCLUDED.additional_workplace_ids,
    specialties = EXCLUDED.specialties;
END;
$$ LANGUAGE plpgsql;

-- Update onboarding status
CREATE OR REPLACE FUNCTION update_onboarding_status(
    p_doctor_account_id integer,
    p_status onboarding_status_enum,
    p_status_reason text
) RETURNS void AS $$
BEGIN
  UPDATE onboarding
  SET status = p_status,
      status_reason = p_status_reason
  WHERE doctor_account_id = p_doctor_account_id;
END;
$$ LANGUAGE plpgsql;

