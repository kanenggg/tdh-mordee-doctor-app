pub(crate) mod gateway;
pub(crate) mod handler;
pub mod repo;
pub(crate) mod service;

use std::sync::Arc;

use axum::{extract::Path, extract::State, Json, Router};
use sqlx::PgPool;

pub use gateway::{JadePolicy, PastVisitGateway};
pub use handler::GetPastVisitDetailResult;
pub use service::PastVisitDetailService;

use repo::{DoctorBasicRepo, DoctorBasicRepoTrait};

pub async fn get_past_visit_detail(
    state: State<Arc<PastVisitDetailService>>,
    booking_id: Path<String>,
    request_id: crate::core::RequestId,
) -> crate::core::error::AppResult<Json<GetPastVisitDetailResult>> {
    handler::get_past_visit_detail(state, booking_id, request_id).await
}

pub fn routes(svc: Arc<PastVisitDetailService>) -> Router {
    handler::routes(svc)
}

pub fn build_service(
    biz_apm_base_uri: String,
    biz_jade_base_uri: String,
    pg_pool: PgPool,
) -> Arc<PastVisitDetailService> {
    let gateway = Arc::new(PastVisitGateway::new(
        biz_apm_base_uri,
        biz_jade_base_uri,
        JadePolicy::default(),
    ));
    let doctor_repo: Arc<dyn DoctorBasicRepoTrait> = Arc::new(DoctorBasicRepo::new(pg_pool));
    Arc::new(PastVisitDetailService::new(gateway, doctor_repo))
}

pub fn router(biz_apm_base_uri: String, biz_jade_base_uri: String, pg_pool: PgPool) -> Router {
    handler::routes(build_service(biz_apm_base_uri, biz_jade_base_uri, pg_pool))
}
