use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultationPreScreen {
    pub symptom: String,
    pub duration: i32,
    pub duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}
