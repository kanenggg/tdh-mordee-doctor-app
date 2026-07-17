use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdentity {
    pub account_id: i32,
    pub account_type: i32,
    pub user_profile_id: i32,
    pub user_main_profile_id: i32,
    pub tenant_id: i32,
    pub oidc_user_id: Option<String>,
    pub legacy_data: Option<LegacyData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyData {
    pub user_id: String,
    pub uid: i32,
    pub client_id: String,
    pub client_int_id: i32,
    pub scopes: String,
    pub role_code: String,
}
