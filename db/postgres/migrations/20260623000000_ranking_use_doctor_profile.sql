ALTER TABLE doctor_rating
    DROP CONSTRAINT IF EXISTS fk_doctor_rating_doctor;

ALTER TABLE doctor_rating
    DROP CONSTRAINT IF EXISTS fk_doctor_rating_doctor_profile;

ALTER TABLE doctor_rating
    ADD CONSTRAINT fk_doctor_rating_doctor_profile
        FOREIGN KEY (doctor_id) REFERENCES doctor_profile (doctor_id) ON DELETE CASCADE NOT VALID;

ALTER TABLE doctor_score
    DROP CONSTRAINT IF EXISTS fk_doctor_score_doctor;

ALTER TABLE doctor_score
    DROP CONSTRAINT IF EXISTS fk_doctor_score_doctor_profile;

ALTER TABLE doctor_score
    ADD CONSTRAINT fk_doctor_score_doctor_profile
        FOREIGN KEY (doctor_id) REFERENCES doctor_profile (doctor_id) ON DELETE CASCADE NOT VALID;

ALTER TABLE doctor_duration
    DROP CONSTRAINT IF EXISTS fk_doctor_duration_doctor;
