pub mod fcm_token;
pub mod handlers;
pub mod repo;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::repo::firestore_repo::FirestoreRepo;

pub use repo::{
    NotificationDoc, NotificationRepo, NotificationRepoTrait, ScheduledNotificationDoc,
    ScheduledNotificationStatus,
};

/// State for notification handlers
#[derive(Clone)]
pub struct NotificationState {
    pub repo: Arc<dyn NotificationRepoTrait>,
}

pub fn router(
    firestore: FirestoreRepo,
    config: &AppConfig,
) -> (Router, Arc<dyn NotificationRepoTrait>) {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(NotificationRepo::new(
        firestore,
        config.firestore.collections.notifications.clone(),
        config.firestore.collections.fcm_tokens.clone(),
    ));

    let state = NotificationState {
        repo: Arc::clone(&repo),
    };

    let r = Router::new()
        .route(
            "/",
            get(handlers::get_notifications).post(handlers::create_notification),
        )
        .route("/read-all", post(handlers::mark_all_as_read))
        .route("/unread-all", post(handlers::mark_all_as_unread))
        .route("/read/{id}", post(handlers::mark_as_read))
        .route(
            "/fcm-token",
            post(handlers::register_fcm_token).get(handlers::get_fcm_tokens),
        )
        .route("/fcm-token/{device_id}", delete(handlers::delete_fcm_token))
        .with_state(state);
    (r, repo)
}
