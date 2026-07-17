pub mod domain;
pub mod handler;
pub mod service;

use std::sync::Arc;

use axum::Router;

pub fn router(service: Arc<dyn service::DoctorCalendarUpdateHandler>) -> Router {
    handler::routes(service)
}
