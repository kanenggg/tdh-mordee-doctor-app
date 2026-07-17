use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Timeslot {
    doctor_id: String,
    slot_start_time: i64,
    duration: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PatientPrescreen {
    symtimp: String,
    symptom_duration: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReseveRequest {
    pub timeslot: Timeslot,
    pub patient_prescreen: PatientPrescreen,
}
