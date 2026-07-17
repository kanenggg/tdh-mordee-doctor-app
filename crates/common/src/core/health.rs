use axum::{Json, Router};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Simple health check response (backward compatible with existing /health endpoint)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthSimpleResponse {
    pub status: String,
}

/// Liveness probe response for Kubernetes
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthLivenessResponse {
    pub status: String,
    #[serde(rename = "__type")]
    pub type_field: String,
}

/// Readiness probe response with dependency health status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthReadinessResponse {
    pub status: String,
    #[serde(rename = "__type")]
    pub type_field: String,
}

/// Detailed health check response with uptime and version
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthDetailedResponse {
    pub status: String,
    #[serde(rename = "__type")]
    pub type_field: String,
    pub uptime_secs: u64,
    pub version: String,
    #[schema(value_type = i64, format = "int64", example = 1710000000)]
    pub timestamp: Timestamp,
}

/// Application startup time (set during initialization)
static mut STARTUP_TIME: Option<Timestamp> = None;

/// Set the application startup time
pub fn set_startup_time() {
    unsafe {
        STARTUP_TIME = Some(Timestamp::now());
    }
}

/// Get the application uptime in seconds
fn get_uptime_secs() -> u64 {
    unsafe {
        if let Some(start) = STARTUP_TIME {
            let now = Timestamp::now();
            let duration = now.since(start).unwrap();
            let whole_secs = duration.get_seconds().max(0);
            whole_secs as u64
        } else {
            0
        }
    }
}

/// Simple health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Health status", body = HealthSimpleResponse)
    )
)]
pub async fn health() -> Json<HealthSimpleResponse> {
    Json(HealthSimpleResponse {
        status: "ok".to_string(),
    })
}

/// Liveness probe endpoint for Kubernetes
#[utoipa::path(
    get,
    path = "/health/liveness",
    tag = "health",
    responses(
        (status = 200, description = "Liveness status", body = HealthLivenessResponse)
    )
)]
pub async fn health_liveness() -> Json<HealthLivenessResponse> {
    Json(HealthLivenessResponse {
        status: "ok".to_string(),
        type_field: "HealthLiveness".to_string(),
    })
}

/// Readiness probe endpoint for Kubernetes
#[utoipa::path(
    get,
    path = "/health/readiness",
    tag = "health",
    responses(
        (status = 200, description = "Readiness status", body = HealthReadinessResponse)
    )
)]
pub async fn health_readiness() -> Json<HealthReadinessResponse> {
    Json(HealthReadinessResponse {
        status: "ok".to_string(),
        type_field: "HealthReadiness".to_string(),
    })
}

/// Detailed health check with uptime and version information
#[utoipa::path(
    get,
    path = "/health/detailed",
    tag = "health",
    responses(
        (status = 200, description = "Detailed health information", body = HealthDetailedResponse)
    )
)]
pub async fn health_detailed() -> Json<HealthDetailedResponse> {
    Json(HealthDetailedResponse {
        status: "ok".to_string(),
        type_field: "HealthDetailed".to_string(),
        uptime_secs: get_uptime_secs(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: Timestamp::now(),
    })
}

/// Kubernetes liveness probe - indicates if the container is alive
#[utoipa::path(
    get,
    path = "/healthz",
    tag = "health",
    responses(
        (status = 200, description = "Container is alive", body = HealthLivenessResponse)
    )
)]
pub async fn healthz_liveness() -> Json<HealthLivenessResponse> {
    Json(HealthLivenessResponse {
        status: "ok".to_string(),
        type_field: "LivenessOk".to_string(),
    })
}

/// Kubernetes readiness probe - indicates if the container is ready to serve traffic
#[utoipa::path(
    get,
    path = "/readyz",
    tag = "health",
    responses(
        (status = 200, description = "Container is ready to serve traffic", body = HealthReadinessResponse),
        (status = 503, description = "Container is not ready", body = HealthReadinessResponse)
    )
)]
pub async fn readiness_probe() -> Json<HealthReadinessResponse> {
    Json(HealthReadinessResponse {
        status: "ok".to_string(),
        type_field: "ReadinessOk".to_string(),
    })
}

/// Create the health check router
pub fn router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(health))
        .route("/health/liveness", axum::routing::get(health_liveness))
        .route("/health/readiness", axum::routing::get(health_readiness))
        .route("/health/detailed", axum::routing::get(health_detailed))
        .route("/healthz", axum::routing::get(healthz_liveness))
        .route("/readyz", axum::routing::get(readiness_probe))
}
