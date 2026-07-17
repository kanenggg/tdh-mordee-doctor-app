CREATE INDEX IF NOT EXISTS idx_consultation_instant_available_doctor
    ON consultation_instant (doctor_id)
    WHERE is_available = true;

CREATE INDEX IF NOT EXISTS idx_consultation_schedule_available_doctor
    ON consultation_schedule (doctor_id)
    WHERE is_available = true;