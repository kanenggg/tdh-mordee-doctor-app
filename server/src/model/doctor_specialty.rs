use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::localize::Localized;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Specialty {
    pub id: i32,
    pub name: Localized,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subspecialty: Option<Box<Specialty>>,
    pub medical_school: MedicalSchool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MedicalSchool {
    pub id: i32,
    pub name: Localized,
}
