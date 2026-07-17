use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailabilityRequest {
    pub available: bool,
    pub biz_unit_id: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityResponse {
    #[serde(rename = "__type")]
    pub response_type: String,
    pub biz_unit_id: i32,
    pub schedule_available: bool,
    pub instant_available: bool,
}

impl AvailabilityResponse {
    pub fn success(biz_unit_id: i32, schedule_available: bool, instant_available: bool) -> Self {
        Self {
            response_type: "Success".to_string(),
            biz_unit_id,
            schedule_available,
            instant_available,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultationAvailability {
    pub schedule_available: bool,
    pub instant_available: bool,
}
