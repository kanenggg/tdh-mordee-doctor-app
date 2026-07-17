CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE OR REPLACE FUNCTION uuid_generate_v7() RETURNS uuid
    LANGUAGE plpgsql
    VOLATILE
AS
$$
DECLARE
    unix_ts_ms bytea;
    uuid_bytes bytea;
BEGIN
    -- 48-bit big-endian Unix epoch in milliseconds (high 6 bytes of int8).
    unix_ts_ms := substring(
            int8send(floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) from 3
                  );
    -- 6 timestamp bytes + 10 random bytes.
    uuid_bytes := unix_ts_ms || gen_random_bytes(10);
    -- Version 7 in the high nibble of byte 6.
    uuid_bytes := set_byte(uuid_bytes, 6,
                           (b'0111' || get_byte(uuid_bytes, 6)::bit(4))::bit(8)::int);
    -- RFC 4122 variant bits (0b10) in the high two bits of byte 8.
    uuid_bytes := set_byte(uuid_bytes, 8,
                           (b'10' || get_byte(uuid_bytes, 8)::bit(6))::bit(8)::int);
    RETURN encode(uuid_bytes, 'hex')::uuid;
END
$$;

ALTER TABLE doctor_profile_draft
    ALTER COLUMN citizen_id TYPE text;

ALTER TABLE doctor_profile_draft
    ADD COLUMN IF NOT EXISTS additional_specialties jsonb DEFAULT '[]'::jsonb;


ALTER TABLE doctor_profile
    DROP CONSTRAINT IF EXISTS doctor_profile_citizen_id_key;

CREATE INDEX IF NOT EXISTS idx_doctor_profile_department_id
    ON doctor_profile (department_id);

CREATE INDEX IF NOT EXISTS idx_doctor_clinic_clinic
    ON doctor_clinic (clinic_id);
CREATE INDEX IF NOT EXISTS idx_doctor_clinic_doctor_id
    ON doctor_clinic (doctor_account_id);
CREATE INDEX IF NOT EXISTS idx_doctor_fee_transaction_account_created
    ON doctor_fee_transaction (doctor_account_id, created_at DESC);


CREATE OR REPLACE FUNCTION reject_doctor_id_change() RETURNS trigger
    LANGUAGE plpgsql
AS
$$
BEGIN
    IF NEW.doctor_id IS DISTINCT FROM OLD.doctor_id THEN
        RAISE EXCEPTION 'doctor_id is immutable (% -> %)', OLD.doctor_id, NEW.doctor_id;
    END IF;
    RETURN NEW;
END
$$;

DROP TRIGGER IF EXISTS trg_doctor_profile_doctor_id_immutable ON doctor_profile;
CREATE TRIGGER trg_doctor_profile_doctor_id_immutable
    BEFORE UPDATE
    ON doctor_profile
    FOR EACH ROW
EXECUTE FUNCTION reject_doctor_id_change();
