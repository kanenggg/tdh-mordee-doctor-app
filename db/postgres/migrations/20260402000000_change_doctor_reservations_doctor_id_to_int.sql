-- Migration: Change doctor_reservations.doctor_id from UUID to INTEGER
-- Reason: doctor_id comes from account_id (integer), not a UUID
-- Date: 2026-04-02

ALTER TABLE doctor_reservations
    ALTER COLUMN doctor_id TYPE INTEGER USING NULL;
