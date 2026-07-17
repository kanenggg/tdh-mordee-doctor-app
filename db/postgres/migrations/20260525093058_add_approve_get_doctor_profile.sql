CREATE FUNCTION get_doctor_profile(p_doctor_account_id integer)
    RETURNS TABLE(citizen_id character varying, profession jsonb, academic_position jsonb, first_name jsonb, last_name jsonb, license_number character varying, medical_school jsonb, specialty jsonb, special_interests text[], address_detail text, sub_district jsonb, district jsonb, province jsonb, postal_code integer, primary_workplace jsonb, additional_workplace jsonb, profile_image_url character varying, id_card_image_url character varying, book_bank_image_url character varying, medical_license_image_url character varying, education_license_image_url text[])
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