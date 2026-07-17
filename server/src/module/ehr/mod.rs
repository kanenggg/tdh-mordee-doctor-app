pub mod lab_result;
pub mod past_visit_detail;
pub mod past_visit_history;

use axum::Router;
use sqlx::PgPool;

pub fn router(
    ehr_base_uri: String,
    biz_apm_base_uri: String,
    biz_jade_base_uri: String,
    pg_pool: PgPool,
) -> Router {
    Router::new()
        .merge(lab_result::router(ehr_base_uri))
        .merge(past_visit_history::router(biz_apm_base_uri.clone()))
        .merge(past_visit_detail::router(
            biz_apm_base_uri,
            biz_jade_base_uri,
            pg_pool,
        ))
}
