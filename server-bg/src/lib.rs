pub mod circuit_breaker;
pub mod config;
pub mod infra;
pub mod module;

use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    pub status: &'static str,
    pub message: String,
}

pub fn build_app(
    calendar_handler: Arc<module::doctor_calendar::service::DoctorCalendarService>,
    doctor_notification_handler: Arc<
        module::doctor_notification::service::DoctorNotificationService,
    >,
    delivery_handler: Arc<module::doctor_notification::service::DoctorNotificationDeliveryService>,
    task_handler: Arc<module::doctor_notification::service::ScheduledNotificationTaskService>,
) -> Router {
    build_app_with_handlers(
        calendar_handler,
        doctor_notification_handler,
        Some(delivery_handler),
        Some(task_handler),
    )
}

pub fn build_app_with_handler<T>(consultation_handler: Arc<T>) -> Router
where
    T: module::doctor_calendar::service::DoctorCalendarUpdateHandler
        + module::doctor_notification::service::DoctorNotificationSendHandler
        + 'static,
{
    build_app_with_handlers(
        consultation_handler.clone(),
        consultation_handler,
        None,
        None,
    )
}

pub fn build_app_with_handlers(
    calendar_handler: Arc<dyn module::doctor_calendar::service::DoctorCalendarUpdateHandler>,
    doctor_notification_handler: Arc<
        dyn module::doctor_notification::service::DoctorNotificationSendHandler,
    >,
    delivery_handler: Option<
        Arc<dyn module::doctor_notification::service::DoctorNotificationDeliveryHandler>,
    >,
    task_handler: Option<
        Arc<dyn module::doctor_notification::service::ScheduledNotificationTaskHandler>,
    >,
) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(module::doctor_calendar::router(calendar_handler))
        .merge(module::doctor_notification::router(
            doctor_notification_handler,
        ))
        .merge(module::doctor_notification::delivery_router(
            delivery_handler,
        ))
        .merge(module::doctor_notification::scheduled_notification_router(
            task_handler,
        ))
        .layer(axum::middleware::from_fn(
            common::core::gcp_logging_middleware,
        ))
        .layer(CorsLayer::permissive())
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse {
            status: "ok",
            message: "server-bg is healthy".to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use common::messaging::pubsub_handler::PubsubProcessingOutcome;
    use std::sync::Mutex;
    use tdh_protocol::notification::{PubsubMessage, PubsubPushMessage};

    #[tokio::test]
    async fn health_returns_ok() {
        let response = health().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[derive(Default)]
    struct FakeHandler {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl module::doctor_calendar::service::DoctorCalendarUpdateHandler for FakeHandler {
        async fn update_doctor_calendar(&self, push: PubsubPushMessage) -> PubsubProcessingOutcome {
            self.calls.lock().unwrap().push(push.message.message_id);
            PubsubProcessingOutcome::Success {
                message: "fake processed".to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl module::doctor_notification::service::DoctorNotificationSendHandler for FakeHandler {
        async fn send_doctor_notification(
            &self,
            push: PubsubPushMessage,
        ) -> PubsubProcessingOutcome {
            self.calls.lock().unwrap().push(push.message.message_id);
            PubsubProcessingOutcome::Success {
                message: "fake processed".to_string(),
            }
        }
    }

    #[tokio::test]
    async fn consultation_event_routes_delegate_pubsub_pushes() {
        let handler = Arc::new(FakeHandler::default());
        let app = build_app_with_handler(handler.clone());
        let server = TestServer::new(app).unwrap();

        server
            .post("/consultation/v1/doctor-calendar/update")
            .json(&PubsubPushMessage {
                message: PubsubMessage {
                    data: "e30=".to_string(),
                    message_id: "calendar-message".to_string(),
                    publish_time: "2026-06-24T00:00:00Z".to_string(),
                    attributes: Default::default(),
                },
                subscription: "doctor-calendar-update-sub".to_string(),
            })
            .await
            .assert_status_ok();

        server
            .post("/consultation/v1/doctor-notifications/send")
            .json(&PubsubPushMessage {
                message: PubsubMessage {
                    data: "e30=".to_string(),
                    message_id: "notification-message".to_string(),
                    publish_time: "2026-06-24T00:00:00Z".to_string(),
                    attributes: Default::default(),
                },
                subscription: "doctor-notification-send-sub".to_string(),
            })
            .await
            .assert_status_ok();

        server
            .post("/consultation/v1/events/status-changed")
            .json(&PubsubPushMessage {
                message: PubsubMessage {
                    data: "e30=".to_string(),
                    message_id: "generic-message".to_string(),
                    publish_time: "2026-06-24T00:00:00Z".to_string(),
                    attributes: Default::default(),
                },
                subscription: "consultation-status-changed-sub".to_string(),
            })
            .await
            .assert_status_ok();

        assert_eq!(
            handler.calls.lock().unwrap().as_slice(),
            [
                "calendar-message".to_string(),
                "notification-message".to_string(),
                "generic-message".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn task_health_route_lives_in_server_bg() {
        let app = build_app_with_handler(Arc::new(FakeHandler::default()));
        let server = TestServer::new(app).unwrap();

        let response = server
            .get(module::doctor_notification::TASK_HEALTH_PATH)
            .await;

        response.assert_status_ok();
    }
}
