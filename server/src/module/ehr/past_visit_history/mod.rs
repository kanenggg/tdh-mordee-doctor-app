pub(crate) mod gateway;
pub(crate) mod handler;
pub(crate) mod service;

use std::sync::Arc;

use axum::Router;

pub use handler::get_past_visits;
pub use service::PastVisitHistoryService;

use gateway::PastVisitGateway;

pub fn build_service(biz_apm_base_uri: String) -> Arc<PastVisitHistoryService> {
    let gateway = Arc::new(PastVisitGateway::new(biz_apm_base_uri));
    Arc::new(PastVisitHistoryService::new(gateway))
}

pub fn router(biz_apm_base_uri: String) -> Router {
    handler::routes(build_service(biz_apm_base_uri))
}
