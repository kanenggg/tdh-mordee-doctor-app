pub mod routes;
pub mod services;
pub mod summarization;

pub use services::ConsultationService;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

use crate::module::timeslot::repo::TimeslotRepo;
use crate::module::webhook::PubsubPublisher;
use summarization::{
    BizApmHttpClient, FollowUpReservationRepoImpl, JadeHttpClient, SummarizationEncryptor,
    SummarizationRepoPsql, SummarizationService, SummarizationState,
};

pub fn router(
    pool: PgPool,
    paseto_key_hex: &str,
    jade_base_uri: String,
    consultation_base_uri: String,
    pubsub_publisher: Arc<PubsubPublisher>,
    consultations_topic: String,
    timeslot_repo: Arc<dyn TimeslotRepo>,
) -> Result<Router, crate::core::error::AppError> {
    // let svc: Arc<ConsultationService> = Arc::new(ConsultationService::new(base_uri));

    let encryptor = Arc::new(SummarizationEncryptor::from_hex(paseto_key_hex)?);
    let summarization_repo: Arc<dyn summarization::SummarizationRepo> =
        Arc::new(SummarizationRepoPsql::new(pool));
    let jade_service: Arc<dyn summarization::JadeServiceTrait> =
        Arc::new(JadeHttpClient::new(jade_base_uri));
    let consultation_summ_service: Arc<dyn summarization::ConsultationSummarizationServiceTrait> =
        Arc::new(BizApmHttpClient::new(consultation_base_uri));
    let follow_up_repo: Arc<dyn summarization::FollowUpReservationRepo> =
        Arc::new(FollowUpReservationRepoImpl::new(timeslot_repo));

    let summarization_service = Arc::new(SummarizationService::new(
        summarization_repo,
        encryptor,
        jade_service,
        consultation_summ_service,
        pubsub_publisher,
        consultations_topic,
        follow_up_repo,
    ));

    let summarization_state = SummarizationState {
        service: summarization_service,
    };

    // let consultation_routes = Router::new()
    //     .route(
    //         "/submit/face-verification",
    //         post(routes::submit_face_verification),
    //     )
    //     .route("/end-session", post(routes::end_session))
    //     .route("/session-info", get(routes::get_session_info))
    //     .with_state(Arc::clone(&svc));

    let summarization_routes = Router::new()
        .route(
            "/summarization/{appointment_id}",
            get(summarization::handler::get_summary_note),
        )
        .route(
            "/summarization/draft",
            post(summarization::handler::save_draft),
        )
        .route(
            "/summarization/submit",
            post(summarization::handler::submit),
        )
        .with_state(summarization_state);

    Ok(summarization_routes)
}
