//! Compatibility delivery for a durably committed DoctorProfile outbox event.
use crate::core::error::{AppError, AppResult};
use crate::module::profile_event_outbox::{
    lease_persisted_event, mark_persisted_event_published, release_persisted_event_after_failure,
};
use crate::module::webhook::PubsubPublisher;
use sqlx::PgPool;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;
use uuid::Uuid;

const IMMEDIATE_DELIVERY_LEASE_SECONDS: i64 = 30;
const IMMEDIATE_DELIVERY_PUBLISH_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ImmediateDoctorProfileDelivery {
    pool: PgPool,
    publisher: Arc<PubsubPublisher>,
    topic: String,
}

impl ImmediateDoctorProfileDelivery {
    pub fn new(pool: PgPool, publisher: Arc<PubsubPublisher>, topic: String) -> Self {
        Self {
            pool,
            publisher,
            topic,
        }
    }

    /// Reads and leases the persisted event. It never reconstructs a payload.
    pub async fn deliver_best_effort(&self, event_id: Uuid) {
        let leased = match lease_persisted_event(
            &self.pool,
            event_id,
            IMMEDIATE_DELIVERY_LEASE_SECONDS,
        )
        .await
        {
            Ok(Some(event)) => event,
            Ok(None) => return,
            Err(error) => {
                warn!(event_id = %event_id, error = %error, "could not lease persisted doctor profile event for immediate delivery");
                return;
            }
        };
        let attributes = HashMap::from([
            ("eventType".to_string(), leased.event_type.clone()),
            ("eventId".to_string(), leased.event_id.to_string()),
            (
                "schemaVersion".to_string(),
                leased.schema_version.to_string(),
            ),
        ]);
        let published = publish_with_timeout(
            IMMEDIATE_DELIVERY_PUBLISH_TIMEOUT,
            self.publisher.publish_with_options(
                &self.topic,
                &leased.payload,
                Some(&leased.doctor_account_id.to_string()),
                Some(attributes),
            ),
        )
        .await;
        match published {
            Ok(_) => {
                if let Err(error) =
                    mark_persisted_event_published(&self.pool, leased.event_id, leased.lease_token)
                        .await
                {
                    warn!(event_id = %leased.event_id, error = %error, "immediate delivery succeeded but its outbox lease could not be completed");
                }
            }
            Err(error) => {
                warn!(event_id = %leased.event_id, error = %error, "immediate doctor profile delivery failed; relay will retry durable event");
                if let Err(release_error) = release_persisted_event_after_failure(
                    &self.pool,
                    leased.event_id,
                    leased.lease_token,
                )
                .await
                {
                    warn!(event_id = %leased.event_id, error = %release_error, "could not release failed immediate delivery lease");
                }
            }
        }
    }
}

async fn publish_with_timeout<T>(
    timeout: Duration,
    publish: impl Future<Output = AppResult<T>>,
) -> AppResult<T> {
    tokio::time::timeout(timeout, publish)
        .await
        .map_err(|_| AppError::PubsubPublishError("immediate delivery timed out".to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timed_out_publish_is_returned_as_a_publish_failure() {
        let result =
            publish_with_timeout(Duration::ZERO, std::future::pending::<AppResult<()>>()).await;

        assert!(matches!(result, Err(AppError::PubsubPublishError(_))));
    }
}
