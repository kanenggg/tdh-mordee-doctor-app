-- Migration: consultation_summarization
-- Creates the table for storing consultation summary drafts and submitted records.
-- The summary_note field is encrypted at rest using Paseto v4 Local (XChaCha20-Poly1305).

CREATE TYPE summarization_status_enum AS ENUM ('Draft', 'Submitted');

CREATE TABLE consultation_summarization (
    appointment_id              VARCHAR(255)                PRIMARY KEY,
    doctor_account_id           INTEGER                     NOT NULL,
    doctor_profile_id           INTEGER                     NOT NULL,
    status                      summarization_status_enum   NOT NULL DEFAULT 'Draft',

    -- Encrypted as a Paseto v4 Local token. NULL = doctor has not filled in summary note yet.
    summary_note_encrypted      TEXT,

    -- Stored as plain JSONB (not sensitive).
    prescription_items          JSONB,
    follow_up_info              JSONB,

    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_consultation_summarization_doctor
    ON consultation_summarization (doctor_account_id);
