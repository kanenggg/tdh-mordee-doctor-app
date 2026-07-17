use serde::{Deserialize, Serialize};

use crate::{
    common::meeting_provider::MeetingProvider, consultation::channel::ConsultationChannel,
};
//
// #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub enum StartingType {
//     StartLater,
//     StartNow,
// }

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    RoomCreated,
    Started,
    Ended,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum GetDoctorSessionInfoResult {
    #[serde(rename = "GetDoctorSessionInfo.SessionReady")]
    SessionReady(SessionReady),
    #[serde(rename = "GetDoctorSessionInfo.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "GetDoctorSessionInfo.SessionIsFinished")]
    SessionIsFinished,
    #[serde(rename = "GetDoctorSessionInfo.SessionIsNotReady")]
    SessionIsNotReady,
    #[serde(rename = "GetDoctorSessionInfo.ProviderIsOutOfService")]
    ProviderIsOutOfService(MeetingProvider),
    #[serde(rename = "getdoctorsessioninfo.unauthorized")]
    Unauthorized,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum ProviderSessionInfo {
    TokBox(TokBoxSessionInfo),
    Twilio(TwilioSessionInfo),
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokBoxSessionInfo {
    pub conference_provider_id: i32,
    pub session_id: String,
    pub session_token: String,
    pub appointment_no: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwilioSessionInfo {
    pub session_name: String,
    pub session_chat_name: Option<String>,
    pub session_token: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReady {
    pub session_info: ProviderSessionInfo,
    pub session_start_time: i64,
    pub session_end_time: i64,
    pub is_facial_verified: bool,
    pub is_required_patient_verification: Option<bool>,
    pub session_channel: ConsultationChannel,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProviderNotSupport {
    pub privider_name: MeetingProvider,
}
