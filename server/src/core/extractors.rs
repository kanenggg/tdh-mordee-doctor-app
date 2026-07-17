use axum::{extract::FromRequestParts, http::request::Parts};
use std::future::Future;

use crate::core::error::{AppError, AppResult};
use crate::core::RequestId;

impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .ok_or(AppError::InternalError(
                "RequestId extension missing — ensure gcp_logging_middleware is registered"
                    .to_string(),
            ));
        async move { result }
    }
}

#[derive(Debug, Clone)]
pub struct PatientHeaders {
    pub patient_account_id: i32,
    pub patient_profile_id: i32,
}

fn parse_header_i32(parts: &Parts, name: &str, header_key: &str) -> AppResult<i32> {
    let value = parts
        .headers
        .get(header_key)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest(format!("missing/invalid {} header", name)))?;

    value
        .parse::<i32>()
        .map_err(|_| AppError::BadRequest(format!("missing/invalid {} header", name)))
}

impl<S> FromRequestParts<S> for PatientHeaders
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let patient_account_id =
            parse_header_i32(parts, "PATIENT-ACCOUNT-ID", "patient-account-id");
        let patient_profile_id =
            parse_header_i32(parts, "PATIENT-PROFILE-ID", "patient-profile-id");

        async move {
            Ok(PatientHeaders {
                patient_account_id: patient_account_id?,
                patient_profile_id: patient_profile_id?,
            })
        }
    }
}
