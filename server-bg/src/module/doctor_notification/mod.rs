pub mod domain;
pub mod handler;
pub mod service;

use std::sync::Arc;

use axum::Router;

pub const SEND_PATH: &str = "/consultation/v1/doctor-notifications/send";
pub const DELIVERY_PATH: &str = "/notifications/v1/send";
pub const SCHEDULED_NOTIFICATION_TASK_PATH: &str = "/tasks/v1/notification";
pub const TASK_HEALTH_PATH: &str = "/tasks/v1/health";

pub fn router(service: Arc<dyn service::DoctorNotificationSendHandler>) -> Router {
    handler::routes(service)
}

pub fn delivery_router(
    service: Option<Arc<dyn service::DoctorNotificationDeliveryHandler>>,
) -> Router {
    handler::delivery_routes(service)
}

pub fn scheduled_notification_router(
    service: Option<Arc<dyn service::ScheduledNotificationTaskHandler>>,
) -> Router {
    handler::scheduled_notification_routes(service)
}
