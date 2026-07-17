-- Migration: Add doctor_reservations table
-- This replaces the old reservations table for dynamic timeslot generation
-- Author: Doctor Actor Refactoring
-- Date: 2026-04-01

-- Create doctor_reservations table
CREATE TABLE IF NOT EXISTS doctor_reservations (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doctor_id           UUID NOT NULL,
    patient_id          UUID,
    slot_date           DATE NOT NULL,
    start_time          TIME NOT NULL,
    end_time            TIME NOT NULL,
    status              reservation_status_enum NOT NULL DEFAULT 'Pending',
    correlation_id      TEXT,
    source              TEXT NOT NULL DEFAULT 'booking',  -- 'booking' | 'follow_up'
    expires_at          TIMESTAMPTZ,
    confirmed_at        TIMESTAMPTZ,
    cancelled_at        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Add indexes for performance
CREATE INDEX IF NOT EXISTS idx_doc_res_doctor_date
    ON doctor_reservations(doctor_id, slot_date);

CREATE INDEX IF NOT EXISTS idx_doc_res_expires
    ON doctor_reservations(expires_at)
    WHERE status = 'Pending';

CREATE UNIQUE INDEX IF NOT EXISTS idx_doc_res_correlation
    ON doctor_reservations(correlation_id)
    WHERE correlation_id IS NOT NULL;

-- Create enum type if not exists
DO $$
BEGIN
    CREATE TYPE reservation_status_enum AS ENUM (
        'Pending',
        'Confirmed',
        'Cancelled',
        'Expired'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Add comments
COMMENT ON TABLE doctor_reservations IS 'Doctor reservations for timeslot booking and follow-up scheduling';
COMMENT ON COLUMN doctor_reservations.source IS 'Source of reservation: booking (patient-initiated) or follow_up (doctor-scheduled)';
COMMENT ON COLUMN doctor_reservations.patient_id IS 'Patient ID - NULL for follow-up reservations before patient acceptance';
