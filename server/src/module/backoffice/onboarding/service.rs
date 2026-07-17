use crate::core::error::{AppError, AppResult};
use crate::module::backoffice::consultation_configuration::{
    build_doctor_configuration, ConsultationConfigInfo,
};
use crate::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery;
use crate::module::backoffice::onboarding::repo::BackofficeOnboardingRepo;
use std::sync::Arc;
use tdh_protocol::doctor_profile::DoctorProfileEvent;
use tracing::{error, info};
use uuid::Uuid;

pub struct BackofficeOnboardingService {
    repo: BackofficeOnboardingRepo,
    immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
}

pub struct ApproveDoctorInfo {
    pub doctor_account_id: i32,
    pub department_id: i32,
    pub clinics: Vec<i32>,
    pub consultation_config: ConsultationConfigInfo,
}

impl BackofficeOnboardingService {
    pub fn new(
        repo: BackofficeOnboardingRepo,
        immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
    ) -> Self {
        Self {
            repo,
            immediate_delivery,
        }
    }

    pub async fn approve_doctor(
        &self,
        request_id: &str,
        request: ApproveDoctorInfo,
    ) -> AppResult<()> {
        let doctor_account_id = request.doctor_account_id;
        let department_id = request.department_id;
        let config = build_doctor_configuration(request.consultation_config)?;

        let approved = self
            .repo
            .approve_with_setup(
                doctor_account_id,
                doctor_account_id,
                department_id,
                &config,
                &request.clinics,
            )
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    department_id,
                    request_id,
                    service = "BackofficeOnboardingService",
                    error = ?e,
                    "approve_doctor failed"
                );
            })?;

        let Some(mutation) = approved else {
            return Err(AppError::BadRequest(
                "Onboarding info not found".to_string(),
            ));
        };

        self.deliver_committed_event(mutation.event.as_ref()).await;

        info!(doctor_account_id, request_id, "doctor approved");
        Ok(())
    }

    pub async fn reject_doctor(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        reason: String,
    ) -> AppResult<()> {
        self.repo
            // TEMPORARY: attribute to the subject doctor_account_id until back-office identity exists.
            .reject_onboarding(doctor_account_id, reason, doctor_account_id)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    request_id,
                    service = "BackofficeOnboardingService",
                    error = ?e,
                    "reject_doctor failed"
                );
            })?;
        info!(doctor_account_id, request_id, "doctor rejected");
        Ok(())
    }
}

impl BackofficeOnboardingService {
    async fn deliver_committed_event(&self, event: Option<&DoctorProfileEvent>) {
        let (Some(delivery), Some(event)) = (&self.immediate_delivery, event) else {
            return;
        };
        let Ok(event_id) = Uuid::parse_str(event.event_id()) else {
            return;
        };
        delivery.deliver_best_effort(event_id).await;
    }
}
