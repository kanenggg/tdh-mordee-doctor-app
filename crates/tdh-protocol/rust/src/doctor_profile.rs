use crate::common::Localized;
use crate::consultation::ConsultationChannel;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profession {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcademicPosition {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPlace {
    pub id: i32,
    pub name: Localized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicalSchool {
    pub id: i32,
    pub name: Localized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Specialty {
    pub id: i32,
    pub name: Localized,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subspecialty: Option<Box<Specialty>>,
    pub medical_school: MedicalSchool,
}

fn empty_localized() -> Localized {
    Localized {
        th: String::new(),
        en: String::new(),
    }
}

impl Default for Profession {
    fn default() -> Self {
        Self {
            id: 0,
            name: empty_localized(),
            abbr: empty_localized(),
        }
    }
}

impl Default for AcademicPosition {
    fn default() -> Self {
        Self {
            id: 0,
            name: empty_localized(),
            abbr: empty_localized(),
        }
    }
}

impl Default for MedicalSchool {
    fn default() -> Self {
        Self {
            id: 0,
            name: empty_localized(),
        }
    }
}

impl Default for Specialty {
    fn default() -> Self {
        Self {
            id: 0,
            name: empty_localized(),
            subspecialty: None,
            medical_school: MedicalSchool::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageCode {
    Th,
    En,
}

/// Language supported by a doctor in the complete V2 consultation snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsultationLanguage {
    Th,
    En,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationConfig {
    pub channels: Vec<ConsultationChannel>,
    pub languages: Vec<ConsultationLanguage>,
    pub duration_minutes: i32,
    pub fee_amount: String,
    pub currency: String,
}

impl ConsultationConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.channels.is_empty() || self.languages.is_empty() {
            return Err("channels and languages must be non-empty");
        }
        if !matches!(self.duration_minutes, 15 | 25 | 50) {
            return Err("durationMinutes must be one of 15, 25, or 50");
        }
        if self.currency.trim().is_empty() {
            return Err("currency must be non-empty");
        }
        let (whole, fraction) = self
            .fee_amount
            .split_once('.')
            .ok_or("feeAmount must be fixed-scale")?;
        if whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.len() != 2
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("feeAmount must be a two-decimal numeric string");
        }
        Ok(())
    }
}

/// Lifecycle events for a doctor profile, published to the `doctor-profile` topic.
/// V2 fields are additive under the stable existing discriminators.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum DoctorProfileEvent {
    #[serde(rename_all = "camelCase")]
    DoctorProfileApproved {
        event_id: String,
        doctor_id: Uuid,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        department_id: i32,
        #[serde(default = "empty_localized")]
        department: Localized,
        #[serde(default)]
        counseling_areas: Vec<Localized>,
        is_active: bool,
        #[serde(default)]
        profession: Profession,
        #[serde(default)]
        specialty: Specialty,
        #[serde(default)]
        work_place: Vec<WorkPlace>,
        #[serde(default)]
        academic_position: AcademicPosition,
        first_name: Localized,
        last_name: Localized,
        profile_image_url: String,
        approved_at: i64,
        occurred_at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema_version: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile_version: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        consultation_config: Option<ConsultationConfig>,
    },
    #[serde(rename_all = "camelCase")]
    DoctorProfileStatusUpdated {
        event_id: String,
        doctor_id: String,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        is_active: bool,
        status_updated_at: i64,
        occurred_at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema_version: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile_version: Option<i64>,
    },
}

impl DoctorProfileEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::DoctorProfileApproved { .. } => "DoctorProfileApproved",
            Self::DoctorProfileStatusUpdated { .. } => "DoctorProfileStatusUpdated",
        }
    }

    pub fn schema_version(&self) -> i32 {
        match self {
            Self::DoctorProfileApproved { schema_version, .. }
            | Self::DoctorProfileStatusUpdated { schema_version, .. } => {
                schema_version.unwrap_or(1)
            }
        }
    }

    pub fn event_id(&self) -> &str {
        match self {
            Self::DoctorProfileApproved { event_id, .. }
            | Self::DoctorProfileStatusUpdated { event_id, .. } => event_id,
        }
    }

    pub fn doctor_id(&self) -> String {
        match self {
            Self::DoctorProfileApproved { doctor_id, .. } => doctor_id.to_string(),
            Self::DoctorProfileStatusUpdated { doctor_id, .. } => doctor_id.clone(),
        }
    }

    pub fn doctor_account_id(&self) -> i32 {
        match self {
            Self::DoctorProfileApproved {
                doctor_account_id, ..
            }
            | Self::DoctorProfileStatusUpdated {
                doctor_account_id, ..
            } => *doctor_account_id,
        }
    }

    pub fn profile_version(&self) -> Option<i64> {
        match self {
            Self::DoctorProfileApproved {
                profile_version, ..
            }
            | Self::DoctorProfileStatusUpdated {
                profile_version, ..
            } => *profile_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn v2_snapshot_round_trip_omits_flattened_consultation_fields() {
        let json = serde_json::json!({
            "__type": "DoctorProfileApproved", "eventId": "evt-1",
            "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479", "doctorAccountId": 2443,
            "doctorProfileId": 8891, "departmentId": 17,
            "department": {"th":"อายุรกรรม","en":"Internal Medicine"},
            "counselingAreas": [{"th":"หัวใจ","en":"Heart"}], "isActive": true,
            "profession": {"id":1,"name":{"th":"แพทย์","en":"Doctor"},"abbr":{"th":"พญ.","en":"Dr."}},
            "specialty": {"id":10,"name":{"th":"หัวใจ","en":"Cardiology"},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}},
            "workPlace": [{"id":20,"name":{"th":"โรงพยาบาล","en":"Hospital"}}],
            "academicPosition": {"id":2,"name":{"th":"อาจารย์","en":"Lecturer"},"abbr":{"th":"อ.","en":"Lect."}},
            "firstName":{"th":"สมชาย","en":"Somchai"}, "lastName":{"th":"ใจดี","en":"Jaidee"},
            "profileImageUrl":"https://x/doctor.jpg",
            "approvedAt":1718668800, "occurredAt":1718668800, "schemaVersion":2, "profileVersion":1,
            "consultationConfig":{"channels":["voice","chat"],"languages":["th","en"],"durationMinutes":15,"feeAmount":"650.00","currency":"THB"}
        });
        let event: DoctorProfileEvent = serde_json::from_value(json).unwrap();
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["__type"], "DoctorProfileApproved");
        assert_eq!(value["department"]["en"], "Internal Medicine");
        assert_eq!(value["schemaVersion"], 2);
        assert_eq!(value["consultationConfig"]["feeAmount"], "650.00");
        for duplicate in [
            "doctorFee",
            "doctorFeeCurrency",
            "languages",
            "durationMinutes",
            "channels",
        ] {
            assert!(value.get(duplicate).is_none(), "unexpected {duplicate}");
        }
    }

    #[test]
    fn expanded_v2_payload_with_flattened_fields_still_deserializes() {
        let json = serde_json::json!({
            "__type": "DoctorProfileApproved", "eventId": "evt-1",
            "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479", "doctorAccountId": 2443,
            "doctorProfileId": 8891, "departmentId": 17,
            "department": {"th":"อายุรกรรม","en":"Internal Medicine"},
            "counselingAreas": [{"th":"หัวใจ","en":"Heart"}], "isActive": true,
            "profession": {"id":1,"name":{"th":"แพทย์","en":"Doctor"},"abbr":{"th":"พญ.","en":"Dr."}},
            "specialty": {"id":10,"name":{"th":"หัวใจ","en":"Cardiology"},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}},
            "workPlace": [{"id":20,"name":{"th":"โรงพยาบาล","en":"Hospital"}}],
            "academicPosition": {"id":2,"name":{"th":"อาจารย์","en":"Lecturer"},"abbr":{"th":"อ.","en":"Lect."}},
            "firstName":{"th":"สมชาย","en":"Somchai"}, "lastName":{"th":"ใจดี","en":"Jaidee"},
            "profileImageUrl":"https://x/doctor.jpg", "doctorFee":650, "doctorFeeCurrency":"THB",
            "languages":["th","en"], "durationMinutes":15, "channels":["voice","chat"],
            "approvedAt":1718668800, "occurredAt":1718668800, "schemaVersion":2, "profileVersion":1,
            "consultationConfig":{"channels":["voice","chat"],"languages":["th","en"],"durationMinutes":15,"feeAmount":"650.00","currency":"THB"}
        });
        let event: DoctorProfileEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.schema_version(), 2);
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["consultationConfig"]["feeAmount"], "650.00");
        assert!(value.get("doctorFee").is_none());
        assert!(value.get("doctorFeeCurrency").is_none());
        assert!(value.get("languages").is_none());
        assert!(value.get("durationMinutes").is_none());
        assert!(value.get("channels").is_none());
    }

    #[test]
    fn old_v1_payload_without_added_wire_fields_deserializes() {
        let value = serde_json::json!({
            "__type":"DoctorProfileApproved", "eventId":"evt-1",
            "doctorId":"f47ac10b-58cc-4372-a567-0e02b2c3d479", "doctorAccountId":1,
            "doctorProfileId":2, "departmentId":3, "isActive":true,
            "firstName":{"th":"a","en":"b"}, "lastName":{"th":"c","en":"d"},
            "profileImageUrl":"u", "approvedAt":0, "occurredAt":0
        });
        let event: DoctorProfileEvent = serde_json::from_value(value).unwrap();
        assert_eq!(event.schema_version(), 1);
        assert_eq!(event.profile_version(), None);
    }

    #[test]
    fn status_updated_payload_has_one_shape_for_both_states() {
        let payload = |is_active| {
            serde_json::json!({
                "__type": "DoctorProfileStatusUpdated",
                "eventId": "evt-status",
                "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
                "doctorAccountId": 2443,
                "doctorProfileId": 8891,
                "isActive": is_active,
                "statusUpdatedAt": 1718668800,
                "occurredAt": 1718668800,
                "schemaVersion": 3,
                "profileVersion": 2
            })
        };

        let active = serde_json::to_value(
            serde_json::from_value::<DoctorProfileEvent>(payload(true)).unwrap(),
        )
        .unwrap();
        let inactive = serde_json::to_value(
            serde_json::from_value::<DoctorProfileEvent>(payload(false)).unwrap(),
        )
        .unwrap();

        let active_keys = active.as_object().unwrap().keys().collect::<BTreeSet<_>>();
        let inactive_keys = inactive
            .as_object()
            .unwrap()
            .keys()
            .collect::<BTreeSet<_>>();
        assert_eq!(active_keys, inactive_keys);
        assert_eq!(active["__type"], "DoctorProfileStatusUpdated");
        assert_eq!(active["isActive"], true);
        assert_eq!(inactive["isActive"], false);
        assert!(active.get("deactivatedAt").is_none());
        assert_eq!(active["statusUpdatedAt"], 1718668800);
    }

    #[test]
    fn deactivated_payload_is_rejected() {
        let json = serde_json::json!({
            "__type": "DoctorProfileDeactivated",
            "eventId": "evt-2",
            "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "doctorAccountId": 2443,
            "doctorProfileId": 8891,
            "deactivatedAt": 1718668800,
            "occurredAt": 1718668800,
            "schemaVersion": 2,
            "profileVersion": 2
        });

        assert!(serde_json::from_value::<DoctorProfileEvent>(json).is_err());
    }

    #[test]
    fn consultation_config_requires_a_complete_snapshot() {
        assert!(ConsultationConfig {
            channels: vec![],
            languages: vec![ConsultationLanguage::En],
            duration_minutes: 30,
            fee_amount: "650".into(),
            currency: String::new(),
        }
        .validate()
        .is_err());
    }
}
