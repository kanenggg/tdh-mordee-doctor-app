use reqwest::{multipart, Client};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;
use utoipa::ToSchema;

use crate::core::error::{AppError, AppResult};

// ── Public types (gateway outward contract — must stay unchanged) ─────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FaceVerificationRequest {
    pub appointment_id: String,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionChannel {
    #[serde(rename = "__type")]
    pub channel_type: String,
    pub session_name: Option<String>,
    pub session_chat_name: Option<String>,
    pub session_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_channel: SessionChannel,
    pub session_start_time: i64,
    pub session_end_time: i64,
    pub is_facial_verified: bool,
    pub is_patient_identity_verified: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetSessionInfoResult {
    SessionInformation(SessionInfo),
    SessionNotFound,
    SessionIsFinished,
    SessionIsNotReady,
    Unauthorized,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum EndSessionResult {
    SessionEnded,
    SessionNotFound,
    Unauthorized,
}

// ── Upstream (consultation v2) response mirrors ───────────────────────────────
//
// Private `Deserialize` mirrors of the upstream OpenAPI tagged unions. They are
// mapped back to the public types above so the gateway's outward contract stays
// unchanged. See docs/superpowers/specs/2026-05-20-consultation-v2-client-spec-alignment-design.md.

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum UpstreamEndSessionResult {
    #[serde(rename = "EndSession.Success")]
    Success,
    #[serde(rename = "EndSession.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "EndSession.Unauthorized")]
    Unauthorized,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum UpstreamGetDoctorSessionInfoResult {
    #[serde(rename = "GetDoctorSessionInfo.SessionReady")]
    SessionReady(UpstreamSessionReady),
    #[serde(rename = "GetDoctorSessionInfo.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "GetDoctorSessionInfo.SessionIsFinished")]
    SessionIsFinished,
    #[serde(rename = "GetDoctorSessionInfo.SessionIsNotReady")]
    SessionIsNotReady,
    #[serde(rename = "GetDoctorSessionInfo.ProviderIsOutOfService")]
    ProviderIsOutOfService,
    // Lowercase in the upstream spec — matched verbatim.
    #[serde(rename = "getdoctorsessioninfo.unauthorized")]
    Unauthorized,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamSessionReady {
    session_info: UpstreamProviderSessionInfo,
    session_start_time: i64,
    session_end_time: i64,
    is_facial_verified: bool,
    // `sessionChannel` and `isRequiredPatientVerification` are intentionally
    // ignored — the public `SessionInfo` has no corresponding fields.
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum UpstreamProviderSessionInfo {
    #[serde(rename = "twilio")]
    Twilio(UpstreamTwilioSessionInfo),
    #[serde(rename = "tokBox")]
    TokBox(UpstreamTokBoxSessionInfo),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamTwilioSessionInfo {
    session_name: String,
    session_token: String,
    session_chat_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamTokBoxSessionInfo {
    session_id: String,
    session_token: String,
    // `conferenceProviderId` and `appointmentNo` are dropped — the public
    // `SessionChannel` has no field for them (accepted limitation).
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum UpstreamAddConsultationScreenshot {
    #[serde(rename = "AddConsultationScreenshot.UploadSuccess")]
    UploadSuccess,
    #[serde(rename = "AddConsultationScreenshot.ScreenshotAlreadyUploaded")]
    ScreenshotAlreadyUploaded,
    #[serde(rename = "AddConsultationScreenshot.ConsultationNotFound")]
    ConsultationNotFound,
    #[serde(rename = "AddConsultationScreenshot.Unauthorized")]
    Unauthorized,
}

// ── Mappings: upstream → public ──────────────────────────────────────────────

fn map_end_session(u: UpstreamEndSessionResult) -> EndSessionResult {
    match u {
        UpstreamEndSessionResult::Success => EndSessionResult::SessionEnded,
        UpstreamEndSessionResult::SessionNotFound => EndSessionResult::SessionNotFound,
        UpstreamEndSessionResult::Unauthorized => EndSessionResult::Unauthorized,
    }
}

fn map_session_info(u: UpstreamGetDoctorSessionInfoResult) -> GetSessionInfoResult {
    match u {
        UpstreamGetDoctorSessionInfoResult::SessionReady(ready) => {
            GetSessionInfoResult::SessionInformation(ready.into())
        }
        UpstreamGetDoctorSessionInfoResult::SessionNotFound => {
            GetSessionInfoResult::SessionNotFound
        }
        UpstreamGetDoctorSessionInfoResult::SessionIsFinished => {
            GetSessionInfoResult::SessionIsFinished
        }
        UpstreamGetDoctorSessionInfoResult::SessionIsNotReady => {
            GetSessionInfoResult::SessionIsNotReady
        }
        // Provider down ⇒ session not ready (closest existing variant).
        UpstreamGetDoctorSessionInfoResult::ProviderIsOutOfService => {
            GetSessionInfoResult::SessionIsNotReady
        }
        UpstreamGetDoctorSessionInfoResult::Unauthorized => GetSessionInfoResult::Unauthorized,
    }
}

fn map_screenshot(u: UpstreamAddConsultationScreenshot, appointment_id: &str) -> AppResult<()> {
    match u {
        UpstreamAddConsultationScreenshot::UploadSuccess
        // Idempotent: a re-upload is treated as success.
        | UpstreamAddConsultationScreenshot::ScreenshotAlreadyUploaded => Ok(()),
        UpstreamAddConsultationScreenshot::Unauthorized => Err(AppError::Unauthorized),
        UpstreamAddConsultationScreenshot::ConsultationNotFound => Err(AppError::UpstreamError(
            format!("facial-upload: consultation not found for appointment {appointment_id}"),
        )),
    }
}

impl From<UpstreamSessionReady> for SessionInfo {
    fn from(r: UpstreamSessionReady) -> Self {
        SessionInfo {
            session_channel: r.session_info.into(),
            session_start_time: r.session_start_time,
            session_end_time: r.session_end_time,
            is_facial_verified: r.is_facial_verified,
            // The new upstream spec exposes only `isRequiredPatientVerification`,
            // not a "verified" flag, so we never report identity as verified and
            // do not conflate the two concepts.
            is_patient_identity_verified: false,
        }
    }
}

impl From<UpstreamProviderSessionInfo> for SessionChannel {
    fn from(info: UpstreamProviderSessionInfo) -> Self {
        match info {
            UpstreamProviderSessionInfo::Twilio(t) => SessionChannel {
                channel_type: "twilio".to_string(),
                session_name: Some(t.session_name),
                session_chat_name: t.session_chat_name,
                session_token: t.session_token,
            },
            UpstreamProviderSessionInfo::TokBox(t) => SessionChannel {
                channel_type: "tokBox".to_string(),
                session_name: Some(t.session_id),
                session_chat_name: None,
                session_token: t.session_token,
            },
        }
    }
}

// ── Public client ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ConsultationService {
    client: Client,
    base_uri: String,
}

impl ConsultationService {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            base_uri,
        }
    }

    pub async fn submit_face_verification(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        appointment_id: &str,
        image: &str,
    ) -> AppResult<()> {
        let url = format!(
            "{}/v2/consultation/facial-upload/{}",
            self.base_uri, appointment_id
        );
        debug!(
            request_id, doctor_account_id, appointment_id, %url,
            "POST consultation facial-upload"
        );

        // Single text part named `image`, forwarding the value verbatim
        // (no base64 decode — preserves the prior forwarding behavior).
        let form = multipart::Form::new().text("image", image.to_string());
        let resp = self
            .client
            .post(&url)
            .header("X-Request-Id", request_id)
            .multipart(form)
            .send()
            .await?;

        let parsed: UpstreamAddConsultationScreenshot =
            parse_upstream(resp, "facial-upload").await?;
        map_screenshot(parsed, appointment_id)
    }

    pub async fn end_session(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        appointment_id: &str,
    ) -> AppResult<EndSessionResult> {
        let url = format!(
            "{}/v2/consultation/end-session/{}",
            self.base_uri, appointment_id
        );
        debug!(
            request_id, doctor_account_id, appointment_id, %url,
            "POST consultation end-session"
        );

        let resp = self
            .client
            .post(&url)
            .header("X-Request-Id", request_id)
            .send()
            .await?;

        let parsed: UpstreamEndSessionResult = parse_upstream(resp, "end-session").await?;
        Ok(map_end_session(parsed))
    }

    pub async fn get_session_info(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        appointment_id: &str,
    ) -> AppResult<GetSessionInfoResult> {
        let url = format!(
            "{}/v2/consultation/session-info/{}",
            self.base_uri, appointment_id
        );
        debug!(
            request_id, doctor_account_id, appointment_id, %url,
            "GET consultation session-info"
        );

        let resp = self
            .client
            .get(&url)
            .header("X-Request-Id", request_id)
            .send()
            .await?;

        let parsed: UpstreamGetDoctorSessionInfoResult =
            parse_upstream(resp, "session-info").await?;
        Ok(map_session_info(parsed))
    }
}

/// Reads the response body for any non-5xx status and parses it as the upstream
/// typed result union. 5xx → `AppError`; an unparseable body → `AppError`.
async fn parse_upstream<T: DeserializeOwned>(
    resp: reqwest::Response,
    context: &str,
) -> AppResult<T> {
    let status = resp.status();
    if status.is_server_error() {
        return Err(AppError::UpstreamError(format!(
            "{context}: consultation service returned {status}"
        )));
    }
    let text = resp.text().await?;
    serde_json::from_str::<T>(&text).map_err(|e| {
        AppError::UpstreamError(format!(
            "{context}: failed to parse consultation response (status {status}): {e}; body={text}"
        ))
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_end_session_variants() {
        let cases = [
            (json!({ "__type": "EndSession.Success" }), "SessionEnded"),
            (
                json!({ "__type": "EndSession.SessionNotFound" }),
                "SessionNotFound",
            ),
            (
                json!({ "__type": "EndSession.Unauthorized" }),
                "Unauthorized",
            ),
        ];
        for (body, expected) in cases {
            let u: UpstreamEndSessionResult = serde_json::from_value(body).unwrap();
            let mapped = serde_json::to_value(map_end_session(u)).unwrap();
            assert_eq!(mapped["__type"], expected);
        }
    }

    #[test]
    fn maps_session_info_non_ready_variants() {
        let cases = [
            (
                json!({ "__type": "GetDoctorSessionInfo.SessionNotFound" }),
                "SessionNotFound",
            ),
            (
                json!({ "__type": "GetDoctorSessionInfo.SessionIsFinished" }),
                "SessionIsFinished",
            ),
            (
                json!({ "__type": "GetDoctorSessionInfo.SessionIsNotReady" }),
                "SessionIsNotReady",
            ),
            // Provider out of service collapses to the closest existing variant.
            (
                json!({ "__type": "GetDoctorSessionInfo.ProviderIsOutOfService" }),
                "SessionIsNotReady",
            ),
            (
                json!({ "__type": "getdoctorsessioninfo.unauthorized" }),
                "Unauthorized",
            ),
        ];
        for (body, expected) in cases {
            let u: UpstreamGetDoctorSessionInfoResult = serde_json::from_value(body).unwrap();
            let mapped = serde_json::to_value(map_session_info(u)).unwrap();
            assert_eq!(mapped["__type"], expected);
        }
    }

    #[test]
    fn maps_session_ready_twilio() {
        let body = json!({
            "__type": "GetDoctorSessionInfo.SessionReady",
            "sessionInfo": {
                "__type": "twilio",
                "sessionName": "room-1",
                "sessionToken": "tok-1",
                "sessionChatName": "chat-1"
            },
            "sessionStartTime": 100,
            "sessionEndTime": 200,
            "isFacialVerified": true,
            "sessionChannel": "video",
            "isRequiredPatientVerification": true
        });
        let u: UpstreamGetDoctorSessionInfoResult = serde_json::from_value(body).unwrap();
        match map_session_info(u) {
            GetSessionInfoResult::SessionInformation(info) => {
                assert_eq!(info.session_channel.channel_type, "twilio");
                assert_eq!(info.session_channel.session_name.as_deref(), Some("room-1"));
                assert_eq!(
                    info.session_channel.session_chat_name.as_deref(),
                    Some("chat-1")
                );
                assert_eq!(info.session_channel.session_token, "tok-1");
                assert_eq!(info.session_start_time, 100);
                assert_eq!(info.session_end_time, 200);
                assert!(info.is_facial_verified);
                // Always false under the new spec.
                assert!(!info.is_patient_identity_verified);
            }
            other => panic!("expected SessionInformation, got {other:?}"),
        }
    }

    #[test]
    fn maps_session_ready_tokbox_dropping_extra_fields() {
        let body = json!({
            "__type": "GetDoctorSessionInfo.SessionReady",
            "sessionInfo": {
                "__type": "tokBox",
                "conferenceProviderId": 7,
                "sessionId": "sess-9",
                "sessionToken": "tok-9",
                "appointmentNo": "A-1"
            },
            "sessionStartTime": 1,
            "sessionEndTime": 2,
            "isFacialVerified": false,
            "sessionChannel": "chat"
        });
        let u: UpstreamGetDoctorSessionInfoResult = serde_json::from_value(body).unwrap();
        match map_session_info(u) {
            GetSessionInfoResult::SessionInformation(info) => {
                assert_eq!(info.session_channel.channel_type, "tokBox");
                assert_eq!(info.session_channel.session_name.as_deref(), Some("sess-9"));
                assert_eq!(info.session_channel.session_chat_name, None);
                assert_eq!(info.session_channel.session_token, "tok-9");
                assert!(!info.is_patient_identity_verified);
            }
            other => panic!("expected SessionInformation, got {other:?}"),
        }
    }

    #[test]
    fn maps_screenshot_variants() {
        let success: UpstreamAddConsultationScreenshot =
            serde_json::from_value(json!({ "__type": "AddConsultationScreenshot.UploadSuccess" }))
                .unwrap();
        assert!(map_screenshot(success, "bk-1").is_ok());

        let already: UpstreamAddConsultationScreenshot = serde_json::from_value(
            json!({ "__type": "AddConsultationScreenshot.ScreenshotAlreadyUploaded" }),
        )
        .unwrap();
        assert!(map_screenshot(already, "bk-1").is_ok());

        let unauthorized: UpstreamAddConsultationScreenshot =
            serde_json::from_value(json!({ "__type": "AddConsultationScreenshot.Unauthorized" }))
                .unwrap();
        assert!(matches!(
            map_screenshot(unauthorized, "bk-1"),
            Err(AppError::Unauthorized)
        ));

        let not_found: UpstreamAddConsultationScreenshot = serde_json::from_value(
            json!({ "__type": "AddConsultationScreenshot.ConsultationNotFound" }),
        )
        .unwrap();
        assert!(matches!(
            map_screenshot(not_found, "bk-1"),
            Err(AppError::UpstreamError(_))
        ));
    }
}
