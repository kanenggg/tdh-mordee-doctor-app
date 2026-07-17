use async_trait::async_trait;
use common::core::error::{AppError, AppResult};
use common::messaging::PubsubPublisher;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LeasedOutboxEvent {
    pub event_id: Uuid,
    pub doctor_account_id: i32,
    pub event_type: String,
    pub schema_version: i32,
    pub profile_version: i64,
    pub payload: Value,
    pub attempts: i32,
    pub lease_token: Uuid,
}

#[async_trait]
pub trait DoctorProfileOutboxRepo: Send + Sync {
    async fn lease_ready(
        &self,
        limit: i64,
        lease_seconds: i64,
    ) -> AppResult<Vec<LeasedOutboxEvent>>;
    async fn mark_published(&self, event_id: Uuid, lease_token: Uuid) -> AppResult<()>;
    async fn reschedule(
        &self,
        event_id: Uuid,
        lease_token: Uuid,
        delay_seconds: i64,
    ) -> AppResult<()>;
}

pub struct PostgresDoctorProfileOutboxRepo {
    pool: PgPool,
}

impl PostgresDoctorProfileOutboxRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DoctorProfileOutboxRepo for PostgresDoctorProfileOutboxRepo {
    async fn lease_ready(
        &self,
        limit: i64,
        lease_seconds: i64,
    ) -> AppResult<Vec<LeasedOutboxEvent>> {
        let lease_token = Uuid::new_v4();
        Ok(sqlx::query_as(r#"
            WITH ready AS (
                SELECT o.event_id FROM doctor_profile_event_outbox o
                WHERE o.published_at IS NULL AND o.available_at <= now()
                  AND (o.leased_until IS NULL OR o.leased_until <= now())
                  -- A doctor's first unpublished version is the only version
                  -- eligible for lease. This keeps retrying/leased v1 ahead of
                  -- v2 even across concurrent relay workers.
                  AND NOT EXISTS (
                      SELECT 1 FROM doctor_profile_event_outbox older
                      WHERE older.doctor_id = o.doctor_id
                        AND older.published_at IS NULL
                        AND older.profile_version < o.profile_version
                  )
                ORDER BY o.created_at, o.event_id FOR UPDATE SKIP LOCKED LIMIT $1
            )
            UPDATE doctor_profile_event_outbox o
            SET lease_token = $2, leased_until = now() + ($3 * interval '1 second'), attempts = attempts + 1
            FROM ready WHERE o.event_id = ready.event_id
            RETURNING o.event_id, o.doctor_account_id, o.event_type, o.schema_version, o.profile_version, o.payload,
                      o.attempts, o.lease_token
        "#).bind(limit).bind(lease_token).bind(lease_seconds).fetch_all(&self.pool).await?)
    }

    async fn mark_published(&self, event_id: Uuid, lease_token: Uuid) -> AppResult<()> {
        let result = sqlx::query("UPDATE doctor_profile_event_outbox SET published_at = now(), leased_until = NULL, lease_token = NULL, last_error = NULL WHERE event_id = $1 AND lease_token = $2 AND published_at IS NULL")
            .bind(event_id).bind(lease_token).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::InternalError(format!(
                "stale outbox lease while marking {event_id} published"
            )));
        }
        Ok(())
    }

    async fn reschedule(
        &self,
        event_id: Uuid,
        lease_token: Uuid,
        delay_seconds: i64,
    ) -> AppResult<()> {
        let result = sqlx::query("UPDATE doctor_profile_event_outbox SET available_at = now() + ($3 * interval '1 second'), leased_until = NULL, lease_token = NULL, last_error = 'publish failed' WHERE event_id = $1 AND lease_token = $2 AND published_at IS NULL")
            .bind(event_id).bind(lease_token).bind(delay_seconds).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            return Err(AppError::InternalError(format!(
                "stale outbox lease while rescheduling {event_id}"
            )));
        }
        Ok(())
    }
}

#[async_trait]
pub trait DoctorProfileOutboxPublisher: Send + Sync {
    async fn publish(&self, event: &LeasedOutboxEvent) -> AppResult<()>;
}

fn topic_for_event_type<'a>(
    event_type: &str,
    approved_topic: &'a str,
    status_updated_topic: &'a str,
) -> Option<&'a str> {
    match event_type {
        "DoctorProfileApproved" => Some(approved_topic),
        "DoctorProfileStatusUpdated" => Some(status_updated_topic),
        _ => None,
    }
}

pub struct PubsubDoctorProfileOutboxPublisher {
    publisher: Arc<PubsubPublisher>,
    approved_topic: String,
    status_updated_topic: String,
}
impl PubsubDoctorProfileOutboxPublisher {
    pub fn new(
        publisher: Arc<PubsubPublisher>,
        approved_topic: String,
        status_updated_topic: String,
    ) -> Self {
        Self {
            publisher,
            approved_topic,
            status_updated_topic,
        }
    }
}

#[async_trait]
impl DoctorProfileOutboxPublisher for PubsubDoctorProfileOutboxPublisher {
    async fn publish(&self, event: &LeasedOutboxEvent) -> AppResult<()> {
        let attributes = HashMap::from([
            ("eventType".to_string(), event.event_type.clone()),
            ("eventId".to_string(), event.event_id.to_string()),
            (
                "schemaVersion".to_string(),
                event.schema_version.to_string(),
            ),
        ]);
        let topic = topic_for_event_type(
            &event.event_type,
            &self.approved_topic,
            &self.status_updated_topic,
        )
        .ok_or_else(|| {
            AppError::InternalError(format!(
                "unsupported doctor profile event type: {}",
                event.event_type
            ))
        })?;
        self.publisher
            .publish_with_options(
                topic,
                &event.payload,
                Some(&event.doctor_account_id.to_string()),
                Some(attributes),
            )
            .await?;
        Ok(())
    }
}

pub struct DoctorProfileOutboxRelay {
    repo: Arc<dyn DoctorProfileOutboxRepo>,
    publisher: Arc<dyn DoctorProfileOutboxPublisher>,
    batch_size: i64,
    lease_seconds: i64,
    publish_timeout: Duration,
}

impl DoctorProfileOutboxRelay {
    pub fn new(
        repo: Arc<dyn DoctorProfileOutboxRepo>,
        publisher: Arc<dyn DoctorProfileOutboxPublisher>,
    ) -> Self {
        Self {
            repo,
            publisher,
            batch_size: 10,
            lease_seconds: 60,
            publish_timeout: Duration::from_secs(5),
        }
    }

    pub fn configured(
        mut self,
        batch_size: i64,
        lease_seconds: i64,
        publish_timeout: Duration,
    ) -> Self {
        self.batch_size = batch_size;
        self.lease_seconds = lease_seconds;
        self.publish_timeout = publish_timeout;
        self
    }

    pub async fn run_once(&self) -> AppResult<usize> {
        let events = self
            .repo
            .lease_ready(self.batch_size, self.lease_seconds)
            .await?;
        let count = events.len();
        for event in events {
            match tokio::time::timeout(self.publish_timeout, self.publisher.publish(&event)).await {
                Ok(Ok(())) => {
                    self.repo
                        .mark_published(event.event_id, event.lease_token)
                        .await?;
                }
                Ok(Err(error)) => {
                    warn!(event_id = %event.event_id, error = %sanitize_publish_error(&error.to_string()), "doctor profile outbox publish failed; scheduling retry");
                    self.repo
                        .reschedule(
                            event.event_id,
                            event.lease_token,
                            retry_delay_seconds(event.attempts),
                        )
                        .await?;
                }
                Err(_) => {
                    warn!(event_id = %event.event_id, timeout_seconds = self.publish_timeout.as_secs(), "doctor profile outbox publish timed out; scheduling retry");
                    self.repo
                        .reschedule(
                            event.event_id,
                            event.lease_token,
                            retry_delay_seconds(event.attempts),
                        )
                        .await?;
                }
            }
        }
        Ok(count)
    }
}

fn retry_delay_seconds(attempts: i32) -> i64 {
    2_i64.saturating_pow(attempts.clamp(1, 10) as u32).min(300)
}

fn sanitize_publish_error(error: &str) -> String {
    error.chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::core::error::AppError;
    use std::sync::Mutex;

    #[test]
    fn status_updated_uses_dedicated_topic_and_unsupported_events_have_no_topic() {
        assert_eq!(
            topic_for_event_type(
                "DoctorProfileStatusUpdated",
                "doctor-profile.approved",
                "doctor-profile.status-updated",
            ),
            Some("doctor-profile.status-updated")
        );
        assert_eq!(
            topic_for_event_type(
                "DoctorProfileApproved",
                "doctor-profile.approved",
                "doctor-profile.status-updated",
            ),
            Some("doctor-profile.approved")
        );
        assert_eq!(
            topic_for_event_type(
                "DoctorProfileDeactivated",
                "doctor-profile.approved",
                "doctor-profile.status-updated",
            ),
            None
        );
    }

    #[derive(Default)]
    struct FakeRepo {
        leased: Mutex<Vec<LeasedOutboxEvent>>,
        published: Mutex<Vec<(Uuid, Uuid)>>,
        retried: Mutex<Vec<(Uuid, Uuid, i64)>>,
    }
    #[async_trait]
    impl DoctorProfileOutboxRepo for FakeRepo {
        async fn lease_ready(&self, _: i64, _: i64) -> AppResult<Vec<LeasedOutboxEvent>> {
            Ok(std::mem::take(&mut *self.leased.lock().unwrap()))
        }
        async fn mark_published(&self, id: Uuid, token: Uuid) -> AppResult<()> {
            self.published.lock().unwrap().push((id, token));
            Ok(())
        }
        async fn reschedule(&self, id: Uuid, token: Uuid, delay: i64) -> AppResult<()> {
            self.retried.lock().unwrap().push((id, token, delay));
            Ok(())
        }
    }
    #[derive(Default)]
    struct FakePublisher {
        sent: Mutex<Vec<(Uuid, Value)>>,
        fail: bool,
    }
    #[async_trait]
    impl DoctorProfileOutboxPublisher for FakePublisher {
        async fn publish(&self, event: &LeasedOutboxEvent) -> AppResult<()> {
            self.sent
                .lock()
                .unwrap()
                .push((event.event_id, event.payload.clone()));
            if self.fail {
                Err(AppError::PubsubPublishError("test".into()))
            } else {
                Ok(())
            }
        }
    }
    fn event() -> LeasedOutboxEvent {
        LeasedOutboxEvent {
            event_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            doctor_account_id: 42,
            event_type: "DoctorProfileApproved".into(),
            schema_version: 2,
            profile_version: 1,
            payload: serde_json::json!({"eventId":"11111111-1111-1111-1111-111111111111","occurredAt":123}),
            attempts: 1,
            lease_token: Uuid::new_v4(),
        }
    }
    #[tokio::test]
    async fn retry_keeps_stable_event_identity_and_payload() {
        let repo = Arc::new(FakeRepo {
            leased: Mutex::new(vec![event()]),
            ..Default::default()
        });
        let publisher = Arc::new(FakePublisher {
            fail: true,
            ..Default::default()
        });
        DoctorProfileOutboxRelay::new(repo.clone(), publisher.clone())
            .run_once()
            .await
            .unwrap();
        assert_eq!(publisher.sent.lock().unwrap()[0].0, event().event_id);
        assert_eq!(publisher.sent.lock().unwrap()[0].1["occurredAt"], 123);
        assert_eq!(repo.retried.lock().unwrap()[0].0, event().event_id);
    }
}
