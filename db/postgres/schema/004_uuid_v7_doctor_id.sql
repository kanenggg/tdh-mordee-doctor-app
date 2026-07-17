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
    unix_ts_ms := substring(
        int8send(floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) from 3
    );
    uuid_bytes := unix_ts_ms || gen_random_bytes(10);
    uuid_bytes := set_byte(uuid_bytes, 6,
        (b'0111' || get_byte(uuid_bytes, 6)::bit(4))::bit(8)::int);
    uuid_bytes := set_byte(uuid_bytes, 8,
        (b'10' || get_byte(uuid_bytes, 8)::bit(6))::bit(8)::int);
    RETURN encode(uuid_bytes, 'hex')::uuid;
END
$$;

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
    BEFORE UPDATE ON doctor_profile
    FOR EACH ROW
EXECUTE FUNCTION reject_doctor_id_change();
