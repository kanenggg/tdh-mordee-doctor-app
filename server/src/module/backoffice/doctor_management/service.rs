use super::repo::DoctorManagementRepo;
use crate::core::error::AppResult;
use crate::module::backoffice::consultation_configuration::{
    build_doctor_configuration, ConsultationConfigInfo,
};
use crate::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery;
use crate::module::profile::configuration::models::UpdateConfigurationResponse;
use std::sync::Arc;
use tdh_protocol::doctor_profile::DoctorProfileEvent;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct DoctorManagementService {
    repo: DoctorManagementRepo,
    approved_immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
    status_updated_immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DoctorProfileDeliveryRoute {
    Approved,
    StatusUpdated,
}

fn delivery_route(event_type: &str) -> Option<DoctorProfileDeliveryRoute> {
    match event_type {
        "DoctorProfileApproved" => Some(DoctorProfileDeliveryRoute::Approved),
        "DoctorProfileStatusUpdated" => Some(DoctorProfileDeliveryRoute::StatusUpdated),
        _ => None,
    }
}

pub struct UpdateConsultationConfigInfo {
    pub doctor_account_id: i32,
    pub consultation_config: ConsultationConfigInfo,
}

impl DoctorManagementService {
    pub fn new(
        repo: DoctorManagementRepo,
        approved_immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
        status_updated_immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
    ) -> Self {
        Self {
            repo,
            approved_immediate_delivery,
            status_updated_immediate_delivery,
        }
    }

    pub async fn update_consultation_config(
        &self,
        request_id: &str,
        action_by: i32,
        request: UpdateConsultationConfigInfo,
    ) -> AppResult<UpdateConfigurationResponse> {
        let doctor_account_id = request.doctor_account_id;
        let config = build_doctor_configuration(request.consultation_config)?;

        let mutation = self
            .repo
            .update_consultation_config(doctor_account_id, action_by, &config)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    action_by,
                    request_id,
                    service = "DoctorManagementService",
                    error = ?e,
                    "update_consultation_config failed"
                );
            })?;

        info!(
            doctor_account_id,
            action_by, request_id, "doctor consultation config updated"
        );

        self.deliver_committed_event(mutation.as_ref().and_then(|change| change.event.as_ref()))
            .await;

        Ok(if mutation.is_some() {
            UpdateConfigurationResponse::Success
        } else {
            UpdateConfigurationResponse::NotFound
        })
    }

    pub async fn update_doctor_active_status(
        &self,
        request_id: &str,
        doctor_account_id: i32,
        is_active: bool,
    ) -> AppResult<()> {
        let event = self
            .repo
            .update_doctor_active_status(doctor_account_id, is_active)
            .await
            .inspect_err(|e| {
                error!(
                    doctor_account_id,
                    is_active,
                    request_id,
                    service = "DoctorManagementService",
                    error = ?e,
                    "update_doctor_active_status failed"
                );
            })?;
        self.deliver_committed_event(event.as_ref()).await;
        info!(
            doctor_account_id,
            is_active, request_id, "doctor status updated"
        );
        Ok(())
    }

    async fn deliver_committed_event(&self, event: Option<&DoctorProfileEvent>) {
        if self.approved_immediate_delivery.is_none()
            && self.status_updated_immediate_delivery.is_none()
        {
            return;
        }
        let Some(event) = event else {
            return;
        };
        let Ok(event_id) = Uuid::parse_str(event.event_id()) else {
            return;
        };
        let pending_events = match self.repo.pending_events_through(event_id).await {
            Ok(events) => events,
            Err(error) => {
                warn!(
                    event_id = %event_id,
                    error = %error,
                    "could not load pending doctor profile events for immediate delivery"
                );
                return;
            }
        };
        for pending in pending_events {
            let delivery = match delivery_route(&pending.event_type) {
                Some(DoctorProfileDeliveryRoute::Approved) => {
                    self.approved_immediate_delivery.as_ref()
                }
                Some(DoctorProfileDeliveryRoute::StatusUpdated) => {
                    self.status_updated_immediate_delivery.as_ref()
                }
                None => continue,
            };
            if let Some(delivery) = delivery {
                delivery.deliver_best_effort(pending.event_id).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_updated_uses_its_topic_and_unsupported_events_have_no_route() {
        assert_eq!(
            delivery_route("DoctorProfileStatusUpdated"),
            Some(DoctorProfileDeliveryRoute::StatusUpdated)
        );
        assert_eq!(
            delivery_route("DoctorProfileApproved"),
            Some(DoctorProfileDeliveryRoute::Approved)
        );
        assert_eq!(delivery_route("DoctorProfileDeactivated"), None);
    }
}
