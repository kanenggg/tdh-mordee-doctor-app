use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum ReserveType {
    DoctorAvailableNow,
    InQueue { consultation_start_at: i64 },
}
