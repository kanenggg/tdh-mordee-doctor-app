pub mod handler;
pub mod repo;
pub mod service;
pub mod validation;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::core::kms::KmsClient;

pub use repo::{OnBoardingRepo, OnBoardingRepoImp};
pub use service::OnboardingService;
pub use validation::OnboardingValidatorImp;

#[derive(Clone)]
pub struct OnboardingState {
    pub service: Arc<OnboardingService>,
}

pub fn router(pool: PgPool, kms: Arc<dyn KmsClient>) -> (Router, Arc<dyn OnBoardingRepo>) {
    let repo: Arc<dyn OnBoardingRepo> = Arc::new(OnBoardingRepoImp::new(pool, kms));
    let service = Arc::new(OnboardingService::new(
        Arc::clone(&repo),
        OnboardingValidatorImp::new(),
    ));

    let state = OnboardingState { service };

    let r = Router::new()
        .route(
            "/",
            get(handler::get_doctor_profile_draft).post(handler::save_doctor_profile_draft),
        )
        .route("/status", get(handler::get_onboarding_status))
        .route("/submit", post(handler::submit_doctor_profile_draft))
        .with_state(state);

    (r, repo)
}
