//! BFF aggregator for the doctor "appointment detail" screen.
//!
//! This module is being rewritten — see
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md
//! and docs/superpowers/plans/2026-04-09-appointment-detail-bff-aggregator.md.

pub mod external;
pub mod handler;
pub mod mapper;
pub mod model;
pub mod service;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::config::AppConfig;
use crate::core::error::AppError;

use self::external::{ConsultationClient, ConsultationClientTrait, IamClient, PaymentClient};
use self::mapper::validate_url_template;
use self::service::{AppointmentService, AppointmentServiceTrait};

pub fn router(cfg: &AppConfig) -> Result<(Router, Arc<dyn ConsultationClientTrait>), AppError> {
    // Startup validation: URL templates must contain exactly one placeholder.
    let insurance_tpl =
        validate_url_template(&cfg.insurance.condition_url_template, "{privilegeId}")
            .map_err(|msg| AppError::InternalError(format!("Invalid insurance config: {}", msg)))?
            .to_string();

    let coupon_tpl = validate_url_template(&cfg.coupon.condition_url_template, "{couponKey}")
        .map_err(|msg| AppError::InternalError(format!("Invalid coupon config: {}", msg)))?
        .to_string();

    let consultation = Arc::new(ConsultationClient::new(
        cfg.service.consultation_base_uri.clone(),
    ));

    let service: Arc<dyn AppointmentServiceTrait> = Arc::new(AppointmentService::new(
        consultation.clone(),
        Arc::new(IamClient::new(cfg.service.iam_gatekeeper_base_uri.clone())),
        Arc::new(PaymentClient::new(
            cfg.service.payment_internal_base_uri.clone(),
        )),
    ));

    let state = handler::AppointmentState {
        service,
        insurance_template: insurance_tpl,
        coupon_template: coupon_tpl,
    };

    let router = Router::new()
        .route("/{bookingId}", get(handler::get_appointment_detail))
        .with_state(state);

    Ok((router, consultation))
}
