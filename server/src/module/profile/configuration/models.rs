use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Consultation channel supported by a doctor. Maps to the Postgres
/// `channel_type_enum` type used by `doctor_consultation_config.channel_types`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "channel_type_enum", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Voice,
    Chat,
    Video,
}

/// Language supported by a doctor. Maps to the Postgres `language_code_enum`
/// type used by `doctor_consultation_config.supported_languages`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "language_code_enum", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum LanguageCode {
    Th,
    En,
}

/// Consultation fee with its currency. `amount` is `null` until the doctor sets
/// a fee (`doctor_consultation_config.doctor_fee_amount` is nullable).
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct Fee {
    pub amount: Option<f64>,
    pub currency: String,
}

/// The doctor's consultation configuration returned by `GET doctor-configuration`.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DoctorConfiguration {
    pub channel: Vec<ChannelType>,
    pub fee: Fee,
    /// Consultation duration in minutes (`doctor_consultation_config.duration_minutes`);
    /// `null` until the doctor sets one.
    pub duration: Option<i32>,
    pub language: Vec<LanguageCode>,
}

/// Response for `GET /profile/v1/doctor-configuration`.
/// Domain "not found" returns HTTP 200 with a `__type` discriminator,
/// matching the project's response convention (see `GetOnboardingResponse`).
#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetDoctorConfigurationResponse {
    #[serde(rename = "DoctorConfigurationResponse")]
    Found(DoctorConfiguration),
    #[serde(rename = "DoctorConfigurationNotFound")]
    NotFound,
}

/// Request body for `PATCH /profile/v1/doctor-channel`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateChannelRequest {
    pub channel: Vec<ChannelType>,
}

/// Request body for `PATCH /profile/v1/doctor-language`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLanguageRequest {
    pub language: Vec<LanguageCode>,
}

/// Response for the PATCH endpoints. `Success` on update; `NotFound` (HTTP 200)
/// when no `doctor_profile` row matches the caller.
#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum UpdateConfigurationResponse {
    Success,
    #[serde(rename = "DoctorConfigurationNotFound")]
    NotFound,
}
