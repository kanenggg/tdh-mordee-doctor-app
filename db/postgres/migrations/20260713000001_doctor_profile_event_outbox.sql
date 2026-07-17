ALTER TABLE doctor_profile
    ADD COLUMN IF NOT EXISTS profile_version bigint NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS doctor_profile_event_outbox
(
    event_id           uuid PRIMARY KEY,
    doctor_id          uuid                     NOT NULL REFERENCES doctor_profile (doctor_id),
    doctor_account_id  integer                  NOT NULL,
    event_type         text                     NOT NULL,
    schema_version     integer                  NOT NULL,
    profile_version    bigint                   NOT NULL,
    occurred_at        timestamptz              NOT NULL,
    payload            jsonb                    NOT NULL,
    attempts           integer                  NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at       timestamptz              NOT NULL DEFAULT now(),
    lease_token        uuid,
    leased_until       timestamptz,
    published_at       timestamptz,
    last_error         text,
    created_at         timestamptz              NOT NULL DEFAULT now(),
    CONSTRAINT doctor_profile_event_outbox_profile_version_key UNIQUE (doctor_id, profile_version)
);

CREATE INDEX IF NOT EXISTS idx_doctor_profile_event_outbox_ready
    ON doctor_profile_event_outbox (available_at)
    WHERE published_at IS NULL;
