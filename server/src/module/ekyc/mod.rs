pub mod handlers;
pub mod service;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::core::gcs_signed_url::GcsSignedUrlGenerator;
use crate::module::appointment::external::ConsultationClientTrait;

use self::handlers::EkycState;
use self::service::{EkycClient, EkycService};

/// Build the eKYC sub-router. Mount under `/appointment/v1`.
pub fn router(
    consultation: Arc<dyn ConsultationClientTrait>,
    eagle_base_uri: String,
    gcp: Arc<dyn GcsSignedUrlGenerator>,
) -> Router {
    let ekyc_client = Arc::new(EkycClient::new(eagle_base_uri));
    let service: Arc<dyn service::EkycServiceTrait> =
        Arc::new(EkycService::new(consultation, ekyc_client, gcp));

    Router::new()
        .route("/{bookingId}/ekyc", get(handlers::get_appointment_ekyc))
        .with_state(EkycState { service })
}

/// Test-only constructor — lets integration tests inject a mock `EkycServiceTrait`
/// without needing real upstream clients.
pub fn router_with_service(service: Arc<dyn service::EkycServiceTrait>) -> Router {
    Router::new()
        .route("/{bookingId}/ekyc", get(handlers::get_appointment_ekyc))
        .with_state(EkycState { service })
}
