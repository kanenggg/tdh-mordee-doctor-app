use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::core::error::AppError;
use crate::core::user_identity::UserIdentity;

const IAM_HEADER: &str = "tdh-sec-iam-user-identity";
const BACKOFFICE_ACCOUNT_TYPE: i32 = 4;

impl<S> FromRequestParts<S> for UserIdentity
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = parts
            .headers
            .get(IAM_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|raw| {
                serde_json::from_str::<UserIdentity>(raw).map_err(|_| AppError::Unauthorized)
            })
            .unwrap_or(Err(AppError::Unauthorized));

        std::future::ready(result)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DoctorIdentity {
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
}

impl<S> FromRequestParts<S> for DoctorIdentity
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = parts
            .headers
            .get(IAM_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|raw| {
                serde_json::from_str::<UserIdentity>(raw)
                    .map_err(|_| AppError::Unauthorized)
                    .and_then(|u| {
                        if u.account_type == 2 || u.account_type == 3 {
                            Ok(DoctorIdentity {
                                doctor_account_id: u.account_id,
                                doctor_profile_id: u.user_main_profile_id,
                            })
                        } else {
                            Err(AppError::Forbidden)
                        }
                    })
            })
            .unwrap_or(Err(AppError::Unauthorized));

        std::future::ready(result)
    }
}

#[derive(Debug, Clone)]
pub struct BackofficeIdentity {
    #[allow(dead_code)]
    pub account_id: i32,
}

impl<S> FromRequestParts<S> for BackofficeIdentity
where
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = parts
            .headers
            .get(IAM_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|raw| {
                serde_json::from_str::<UserIdentity>(raw)
                    .map_err(|_| AppError::Unauthorized)
                    .and_then(|u| {
                        if u.account_type == BACKOFFICE_ACCOUNT_TYPE {
                            Ok(BackofficeIdentity {
                                account_id: u.account_id,
                            })
                        } else {
                            Err(AppError::Unauthorized)
                        }
                    })
            })
            .unwrap_or(Err(AppError::Unauthorized));

        std::future::ready(result)
    }
}

#[cfg(test)]
mod doctor_identity_tests {
    use super::*;
    use axum::extract::FromRequestParts;
    use axum::http::Request;

    #[tokio::test]
    async fn doctor_identity_extracts_profile_id_from_main_profile() {
        let identity = serde_json::json!({
            "accountId": 555,
            "accountType": 2,
            "userProfileId": 900,
            "userMainProfileId": 901,
            "tenantId": 1,
            "oidcUserId": null,
            "legacyData": null
        })
        .to_string();

        let req = Request::builder()
            .header("tdh-sec-iam-user-identity", identity)
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();

        let doctor = DoctorIdentity::from_request_parts(&mut parts, &())
            .await
            .unwrap();
        assert_eq!(doctor.doctor_account_id, 555);
        assert_eq!(doctor.doctor_profile_id, 901);
    }
}
