use std::sync::Arc;
use tracing::{error, info, warn};

use crate::{
    core::error::{AppError, AppResult},
    model::onboarding::{
        OnBoarding, OnBoardingRequest, OnBoardingRequestPatch, OnBoardingStatus, OnBoardingStub,
    },
    module::onboarding::{OnBoardingRepo, OnboardingValidatorImp},
};

#[derive(Clone)]
pub struct OnboardingService {
    repo: Arc<dyn OnBoardingRepo>,
    validator: OnboardingValidatorImp,
}

impl OnboardingService {
    pub fn new(repo: Arc<dyn OnBoardingRepo>, validator: OnboardingValidatorImp) -> Self {
        Self { repo, validator }
    }

    pub async fn get_doctor_profile_draft(
        &self,
        request_id: &str,
        doctor_id: i32,
    ) -> AppResult<Option<OnBoarding>> {
        let stub = self
            .repo
            .get_doctor_profile_draft(doctor_id)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_id,
                    request_id,
                    service = "OnboardingService",
                    error = ?e,
                    "get_doctor_profile_draft failed"
                );
            })?;
        Ok(stub.map(Into::into))
    }

    pub async fn get_onboarding_status(
        &self,
        request_id: &str,
        doctor_account_id: i32,
    ) -> AppResult<Option<OnBoardingStatus>> {
        self.repo
            .get_onboarding_status(doctor_account_id)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    request_id,
                    service = "OnboardingService",
                    error = ?e,
                    "get_onboarding_status failed"
                );
            })
    }

    pub async fn save_doctor_profile_draft(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        patch: OnBoardingRequestPatch,
    ) -> AppResult<()> {
        let mut request = OnBoardingRequest::default();
        request.apply(patch);
        self.repo
            .save_doctor_profile_draft(doctor_account_id, doctor_profile_id, &request)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    doctor_profile_id,
                    request_id,
                    service = "OnboardingService",
                    error = ?e,
                    "save_doctor_profile_draft failed"
                );
            })?;
        info!(doctor_account_id, request_id, "draft saved");
        Ok(())
    }

    pub async fn submit_doctor_profile_draft(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        patch: OnBoardingRequestPatch,
    ) -> AppResult<()> {
        let mut request = OnBoardingRequest::default();
        request.apply(patch);
        let stub = OnBoardingStub::from(request.clone());

        if let Err(e) = self.validator.validate_onboarding_submission(&stub) {
            match &e {
                AppError::ValidationFieldError { field, message } => warn!(
                    doctor_account_id,
                    request_id,
                    field = %field,
                    error = %message,
                    "doctor profile draft failed validation"
                ),
                _ => warn!(
                    doctor_account_id,
                    request_id,
                    error = %e,
                    "doctor profile draft failed validation"
                ),
            }
            return Err(e);
        }

        self.repo
            .submit_doctor_profile_draft(doctor_account_id, doctor_profile_id, &request)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    doctor_profile_id,
                    request_id,
                    service = "OnboardingService",
                    error = ?e,
                    "submit_doctor_profile_draft failed"
                );
            })?;
        info!(
            doctor_account_id,
            request_id, "draft submitted for approval"
        );
        Ok(())
    }
}
