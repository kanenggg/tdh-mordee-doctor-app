/// Lightweight appointment card status for Firebase RTDB.
/// Matches Scala `AppointmentCardStatus` — serialises as `{"__type": "UpComing"}`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(tag = "__type")]
pub enum AppointmentCardStatus {
    #[serde(rename = "Reserved")]
    Reserved,
    UpComing,
    #[serde(rename = "Cancelled")]
    Cancelled,
    #[serde(rename = "Missed")]
    Missed,
    #[serde(rename = "Fail")]
    Fail,
    #[serde(rename = "Completed")]
    Completed,
    #[serde(rename = "PendingRecord")]
    PendingRecord,
    #[serde(rename = "Unknown")]
    Unknown {
        origin: String,
    },
}

impl From<&str> for AppointmentCardStatus {
    fn from(s: &str) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "UPCOMING" | "BOOKED" | "PENDING" | "PROPOSED" => AppointmentCardStatus::UpComing,
            "RESERVED" => AppointmentCardStatus::Reserved,
            "CANCELLED" => AppointmentCardStatus::Cancelled,
            "MISSED" | "NOSHOW" => AppointmentCardStatus::Missed,
            "FAIL" | "FAILED" | "TECHNICALERROR" | "TECHNICAL_ERROR" => AppointmentCardStatus::Fail,
            "COMPLETED" | "FULFILLED" => AppointmentCardStatus::Completed,
            "PENDINGRECORD" | "CONSULTATION_DONE" | "CONSULTATIONDONE" | "ARRIVED"
            | "ENTERED_IN_ERROR" | "ENTERED-IN-ERROR" => AppointmentCardStatus::PendingRecord,
            _ => AppointmentCardStatus::Unknown {
                origin: s.to_string(),
            },
        }
    }
}

impl From<String> for AppointmentCardStatus {
    fn from(s: String) -> Self {
        AppointmentCardStatus::from(s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::AppointmentCardStatus;

    #[test]
    fn consultation_done_maps_to_pending_record() {
        assert_eq!(
            AppointmentCardStatus::from("CONSULTATION_DONE"),
            AppointmentCardStatus::PendingRecord
        );
        assert_eq!(
            AppointmentCardStatus::from("ConsultationDone"),
            AppointmentCardStatus::PendingRecord
        );
    }
}
