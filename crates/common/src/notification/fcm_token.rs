use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FcmTokenDoc {
    #[serde(rename = "fcmToken")]
    pub fcm_token: String,
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub platform: String,
    #[serde(rename = "appVersion")]
    pub app_version: Option<String>,
    #[serde(rename = "registeredAt")]
    #[schema(value_type = String, format = DateTime)]
    pub registered_at: Timestamp,
    #[serde(rename = "lastUsedAt")]
    #[schema(value_type = String, format = DateTime)]
    pub last_used_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterFcmTokenRequest {
    #[serde(rename = "fcmToken")]
    pub fcm_token: String,
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub platform: String,
    #[serde(rename = "appVersion")]
    pub app_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RegisterFcmTokenResponse {
    pub success: bool,
    #[serde(rename = "tokenId")]
    pub token_id: String,
}
