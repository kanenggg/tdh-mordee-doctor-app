//! Handlers for `/appointment/v1/{bookingId}` endpoints.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use jiff::civil::Date;
use tracing::{info, instrument};

use crate::core::auth::DoctorIdentity;
use crate::core::error::AppResult;
use crate::module::ranking::language::LanguagePreference;

use super::mapper;
use super::model::{ApiResponse, PastVisitApiResponse, PastVisitResponse, PastVisitsListResponse};
use super::service::{AppointmentDetailResult, AppointmentServiceTrait, PastVisitsResult};

/// Shared state injected via Axum `State`.
#[derive(Clone)]
pub struct AppointmentState {
    pub service: Arc<dyn AppointmentServiceTrait>,
    pub insurance_template: String,
    pub coupon_template: String,
}

/// `GET /appointment/v1/{bookingId}`
#[utoipa::path(
    get,
    path = "/appointment/v1/{bookingId}",
    tag = "appointment",
    params(
        ("bookingId" = String, Path, description = "Booking ID (e.g. BK20220227810949)")
    ),
    responses(
        (status = 200, description = "Success | AppointmentNotFound | PatientProfileNotFound",
         body = ApiResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (account_type != 3)"),
        (status = 502, description = "Upstream service unavailable"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
#[instrument(
    name = "appointment_detail",
    skip(state),
    fields(booking_id = %booking_id, doctor_account_id = %doctor_identity.doctor_account_id)
)]
pub async fn get_appointment_detail(
    State(state): State<AppointmentState>,
    doctor_identity: DoctorIdentity,
    Path(booking_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let templates = mapper::Templates {
        insurance: &state.insurance_template,
        coupon: &state.coupon_template,
    };
    let today = jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::UTC)
        .date();
    let result = state.service.get_appointment_detail(&booking_id).await?;
    let response = map_detail_response(result, templates, today);

    Ok(Json(response))
}

fn map_detail_response(
    result: AppointmentDetailResult,
    templates: mapper::Templates<'_>,
    today: Date,
) -> ApiResponse {
    match result {
        AppointmentDetailResult::Found(detail) => {
            let body = mapper::compose(
                detail.consultation,
                detail.profile,
                detail.payment,
                templates,
                today,
            );

            info!(
                has_insurance = ?body.payment.as_ref().map(|p| p.has_insurance),
                "appointment detail composed successfully"
            );

            ApiResponse::Success(Box::new(body))
        }
        AppointmentDetailResult::AppointmentNotFound => ApiResponse::AppointmentNotFound,
        AppointmentDetailResult::PatientProfileNotFound => ApiResponse::PatientProfileNotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::{routing::get, Router};
    use axum_test::TestServer;
    use serde_json::json;
    use std::sync::Arc;

    use crate::core::error::AppResult;

    struct PanicService;

    #[async_trait]
    impl AppointmentServiceTrait for PanicService {
        async fn get_appointment_detail(
            &self,
            booking_id: &str,
        ) -> AppResult<AppointmentDetailResult> {
            panic!(
                "appointment service should not be called for {}",
                booking_id
            );
        }
    }

    fn identity_header(account_type: i32) -> String {
        json!({
            "accountId": 300,
            "accountType": account_type,
            "userProfileId": 400,
            "userMainProfileId": 400,
            "tenantId": 1,
            "oidcUserId": null,
            "legacyData": null
        })
        .to_string()
    }

    fn build_server() -> TestServer {
        let state = AppointmentState {
            service: Arc::new(PanicService),
            insurance_template: "https://static.tdh.com/insurance/{insurerKey}.html".to_string(),
            coupon_template: "https://static.tdh.com/coupon/{couponKey}.html".to_string(),
        };

        let app = Router::new()
            .route("/{bookingId}", get(get_appointment_detail))
            .with_state(state);
        TestServer::new(app).unwrap()
    }

    #[tokio::test]
    async fn non_doctor_identity_is_forbidden() {
        let server = build_server();

        let resp = server
            .get("/BK20260227810949")
            .add_header("tdh-sec-iam-user-identity", identity_header(1))
            .await;

        resp.assert_status_forbidden();
    }
}
