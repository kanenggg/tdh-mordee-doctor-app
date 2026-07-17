use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::core::error::{AppError, AppResult};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BizUnitQuery {
    pub biz_unit_id: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum SuccessResponse {
    Success,
}

pub fn validate_biz_unit_id(biz_unit_id: i32) -> AppResult<()> {
    if biz_unit_id <= 0 {
        return Err(AppError::BadRequest(
            "bizUnitId must be greater than 0".to_string(),
        ));
    }

    Ok(())
}
