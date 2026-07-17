//! Response types for appointment endpoints.

use serde::Serialize;
use utoipa::ToSchema;

use crate::model::appointment_status::AppointmentCardStatus;

/// Top-level discriminated response for the appointment-detail endpoint.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum ApiResponse {
    #[serde(rename = "Success")]
    Success(Box<SuccessBody>),
    #[serde(rename = "AppointmentNotFound")]
    AppointmentNotFound,
    #[serde(rename = "PatientProfileNotFound")]
    PatientProfileNotFound,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuccessBody {
    pub booking_id: String,
    pub appointment_no: String,
    pub appointment_time: AppointmentTime,
    /// `YYYY-MM-DD` derived from `appointment_time.start_time` (UTC).
    pub appointment_date: String,
    /// FHIR appointment status, passed through from consultation.
    pub status: AppointmentCardStatus,
    /// `Instant` | `Schedule` | `FollowUp`.
    pub booking_type: String,
    /// `video` | `voice` | `chat`.
    pub consultation_channel: String,
    pub patient: Patient,
    /// `null` when payment-svc returned `NotFound` (no successful payment yet).
    pub payment: Option<Payment>,
    /// `null` when (a) payment is null, (b) upstream couponProtocol is null,
    /// or (c) upstream campaignName is missing or empty.
    pub coupon: Option<Coupon>,
    pub prescreen: Prescreen,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Patient {
    pub account_id: i32,
    pub profile_id: i32,
    pub full_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub age: Option<i32>,
    pub gender: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    pub payment_tx_id: i64,
    pub payment_tx_ref_id: String,
    pub payer_name: String,
    pub has_insurance: bool,
    pub insurance_condition_url: Option<String>,
    /// THB total. Serialized as a JSON number.
    #[schema(value_type = f64)]
    pub amount: serde_json::Number,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Coupon {
    pub campaign_name: String,
    pub condition_url: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Prescreen {
    pub symptom: String,
    pub duration: i32,
    pub duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}

/// Response for `GET /appointment/v1/by-patient/{patientAccountId}/past-visits`.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastVisitsListResponse {
    /// Passthrough array from qolphin (all past visits for the patient).
    #[schema(value_type = Vec<Object>)]
    pub past_visits: Vec<serde_json::Value>,
}

/// Top-level discriminated response for `GET /appointment/v1/{bookingId}/past-visit`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum PastVisitApiResponse {
    #[serde(rename = "Success")]
    Success(PastVisitResponse),
    #[serde(rename = "AppointmentNotFound")]
    AppointmentNotFound,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastVisitResponse {
    /// Single past-visit object passed through from qolphin.
    #[schema(value_type = Object)]
    pub past_visit: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn past_visit_success_serializes_with_past_visit_field() {
        let v = PastVisitApiResponse::Success(PastVisitResponse {
            past_visit: serde_json::json!({ "bookingId": "BK20220101000001", "status": "Completed" }),
        });
        let json: serde_json::Value = serde_json::to_value(&v).unwrap();
        assert_eq!(json["__type"], "Success");
        assert_eq!(json["pastVisit"]["bookingId"], "BK20220101000001");
        assert_eq!(json["pastVisit"]["status"], "Completed");
        assert!(
            json.get("pastVisits").is_none(),
            "must not serialize a list field"
        );
    }

    #[test]
    fn past_visit_appointment_not_found_serializes_with_discriminator_only() {
        let v = PastVisitApiResponse::AppointmentNotFound;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"__type":"AppointmentNotFound"}"#);
    }

    #[test]
    fn appointment_not_found_serializes_with_discriminator_only() {
        let v = ApiResponse::AppointmentNotFound;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"__type":"AppointmentNotFound"}"#);
    }

    #[test]
    fn patient_profile_not_found_serializes_with_discriminator_only() {
        let v = ApiResponse::PatientProfileNotFound;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"__type":"PatientProfileNotFound"}"#);
    }

    #[test]
    fn success_serializes_camel_case_fields() {
        let v = ApiResponse::Success(Box::new(SuccessBody {
            booking_id: "BK20220227810949".to_string(),
            appointment_no: "20220227810949".to_string(),
            appointment_time: AppointmentTime {
                start_time: 1645940400,
                end_time: 1645941300,
            },
            appointment_date: "2022-02-27".to_string(),
            status: AppointmentCardStatus::from("BOOKED"),
            booking_type: "Schedule".to_string(),
            consultation_channel: "video".to_string(),
            patient: Patient {
                account_id: 124236,
                profile_id: 200,
                full_name: Some("Mrs.Bunyang Lopez".to_string()),
                date_of_birth: Some("1957-03-22".to_string()),
                age: Some(45),
                gender: Some("Female".to_string()),
            },
            payment: None,
            coupon: None,
            prescreen: Prescreen {
                symptom: "headache".to_string(),
                duration: 7,
                duration_unit: "day".to_string(),
                attachments: vec![],
                allergies: vec![],
            },
        }));
        let json: serde_json::Value = serde_json::to_value(&v).unwrap();
        // Spot-check key field names use camelCase, not snake_case.
        assert_eq!(json["__type"], "Success");
        assert_eq!(json["bookingId"], "BK20220227810949");
        assert_eq!(json["appointmentNo"], "20220227810949");
        assert_eq!(json["appointmentTime"]["startTime"], 1645940400);
        assert_eq!(json["patient"]["fullName"], "Mrs.Bunyang Lopez");
        assert_eq!(json["status"]["__type"], "UpComing");
        assert_eq!(json["payment"], serde_json::Value::Null);
        assert_eq!(json["coupon"], serde_json::Value::Null);
    }

    #[test]
    fn success_with_payment_and_coupon_serializes_all_camel_case_fields() {
        let v = ApiResponse::Success(Box::new(SuccessBody {
            booking_id: "BK20220227810949".to_string(),
            appointment_no: "20220227810949".to_string(),
            appointment_time: AppointmentTime {
                start_time: 1645940400,
                end_time: 1645941300,
            },
            appointment_date: "2022-02-27".to_string(),
            status: AppointmentCardStatus::from("BOOKED"),
            booking_type: "Schedule".to_string(),
            consultation_channel: "video".to_string(),
            patient: Patient {
                account_id: 124236,
                profile_id: 200,
                full_name: Some("Mrs.Bunyang Lopez".to_string()),
                date_of_birth: Some("1957-03-22".to_string()),
                age: Some(45),
                gender: Some("Female".to_string()),
            },
            payment: Some(Payment {
                payment_tx_id: 1042,
                payment_tx_ref_id: "REF-1042".to_string(),
                payer_name: "AIA".to_string(),
                has_insurance: true,
                insurance_condition_url: Some("https://example.test/aia.html".to_string()),
                amount: serde_json::Number::from_f64(1500.00).unwrap(),
            }),
            coupon: Some(Coupon {
                campaign_name: "New Year Sale 2026".to_string(),
                condition_url: Some("https://example.test/new-year-sale-2026.html".to_string()),
            }),
            prescreen: Prescreen {
                symptom: "headache".to_string(),
                duration: 7,
                duration_unit: "day".to_string(),
                attachments: vec!["att-001".to_string()],
                allergies: vec!["Amoxicillin".to_string()],
            },
        }));
        let json: serde_json::Value = serde_json::to_value(&v).unwrap();

        // Payment camelCase field names
        assert_eq!(json["payment"]["paymentTxId"], 1042);
        assert_eq!(json["payment"]["paymentTxRefId"], "REF-1042");
        assert_eq!(json["payment"]["payerName"], "AIA");
        assert_eq!(json["payment"]["hasInsurance"], true);
        assert_eq!(
            json["payment"]["insuranceConditionUrl"],
            "https://example.test/aia.html"
        );
        assert_eq!(json["payment"]["amount"], 1500.0_f64);

        // Coupon camelCase field names
        assert_eq!(json["coupon"]["campaignName"], "New Year Sale 2026");
        assert_eq!(
            json["coupon"]["conditionUrl"],
            "https://example.test/new-year-sale-2026.html"
        );

        // Prescreen camelCase field names
        assert_eq!(json["prescreen"]["durationUnit"], "day");
        assert_eq!(json["prescreen"]["symptom"], "headache");
        assert_eq!(json["prescreen"]["duration"], 7);
        assert_eq!(json["prescreen"]["attachments"][0], "att-001");
        assert_eq!(json["prescreen"]["allergies"][0], "Amoxicillin");
    }
}
