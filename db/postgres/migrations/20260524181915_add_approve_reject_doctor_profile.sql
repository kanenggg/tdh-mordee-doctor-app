CREATE OR REPLACE FUNCTION approve_doctor_profile_draft(p_doctor_account_id integer, p_action_by integer) RETURNS VOID
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
END IF;

UPDATE doctor_profile_draft
SET status = 'Approved',
    updated_at = now()
WHERE doctor_account_id = p_doctor_account_id
  AND status = 'PendingApproval';

INSERT INTO doctor_profile (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
                            first_name,
                            last_name, license_number, primary_medical_school, specialty, special_interest,
                            address_detail, sub_district, district, province, postal_code, work_place,
                            additional_workplace, profile_image_url, id_card_image_url, book_bank_image_url,
                            medical_license_image_url, education_license_image_url, is_active)
SELECT d.doctor_account_id,
       d.doctor_profile_id,
       d.citizen_id,
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
       d.education_license_image_url,
       'true'
FROM doctor_profile_draft d
WHERE d.doctor_account_id = p_doctor_account_id
  AND d.status = 'Approved';

-- insert transaction
INSERT INTO doctor_profile_transaction (doctor_account_id,status,status_reason,action_by)
VALUES (p_doctor_account_id,'Approved','',p_action_by);
END
$$;

CREATE OR REPLACE FUNCTION reject_doctor_profile_draft(p_doctor_account_id integer, p_reject_reason text, p_action_by integer) RETURNS VOID
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF p_doctor_account_id IS NULL THEN
        RETURN;
END IF;

UPDATE doctor_profile_draft
SET status = 'Rejected',
    updated_at = now()
WHERE doctor_account_id = p_doctor_account_id
  AND status = 'PendingApproval';


-- insert transaction
INSERT INTO doctor_profile_transaction (doctor_account_id,status,status_reason,action_by)
VALUES (p_doctor_account_id,'Rejected',p_reject_reason,p_action_by);
END
$$;