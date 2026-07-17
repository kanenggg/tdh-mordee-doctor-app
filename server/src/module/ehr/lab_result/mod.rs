pub(crate) mod gateway;
pub(crate) mod handler;
pub(crate) mod service;

use std::sync::Arc;

use axum::Router;

pub use handler::get_lab_result;
pub use service::LabResultService;

use gateway::EhrClient;

pub fn build_service(ehr_base_uri: String) -> Arc<LabResultService> {
    let gateway = Arc::new(EhrClient::new(ehr_base_uri));
    Arc::new(LabResultService::new(gateway))
}

pub fn router(ehr_base_uri: String) -> Router {
    handler::routes(build_service(ehr_base_uri))
}
