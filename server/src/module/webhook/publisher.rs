use std::collections::HashMap;

use google_cloud_googleapis::pubsub::v1::PubsubMessage;
use google_cloud_pubsub::client::{Client, ClientConfig};
use serde::Serialize;
use tracing::{debug, info};

use crate::config::PubsubConfig;
use crate::core::error::{AppError, AppResult};
use crate::module::timeslot::model::{
    TimeslotConfirmedEvent, TimeslotReleasedEvent, TimeslotReservedEvent,
};

/// Generic Pub/Sub publisher for sending serializable payloads to topics.
///
/// Holds a long-lived `Client` connection and creates per-call publishers
/// using `publish_immediately` for fail-fast semantics (no batching).
pub struct PubsubPublisher {
    client: Client,
}

impl PubsubPublisher {
    /// Get a reference to the underlying Pub/Sub client (for reuse in subscriber)
    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn new(config: &PubsubConfig) -> AppResult<Self> {
        let use_emulator = config.emulator_host.as_ref().is_some_and(|h| !h.is_empty());

        if use_emulator {
            let host = config.emulator_host.as_ref().unwrap();
            std::env::set_var("PUBSUB_EMULATOR_HOST", host);
            info!(host = %host, "Pub/Sub publisher connecting to emulator");
        }

        let client_config = if use_emulator {
            ClientConfig {
                project_id: Some(config.gcp_project_id.clone()),
                ..ClientConfig::default()
            }
        } else {
            let mut cfg = ClientConfig::default().with_auth().await.map_err(|e| {
                AppError::PubsubPublishError(format!(
                    "Failed to create authenticated Pub/Sub config: {e}"
                ))
            })?;
            if cfg.project_id.is_none() {
                cfg.project_id = Some(config.gcp_project_id.clone());
            }
            cfg
        };

        let client = Client::new(client_config).await.map_err(|e| {
            AppError::PubsubPublishError(format!("Failed to create Pub/Sub client: {e}"))
        })?;

        info!(
            project_id = %config.gcp_project_id,
            emulator = use_emulator,
            "Pub/Sub publisher initialized"
        );

        Ok(Self { client })
    }

    /// Publish a serializable payload to a topic. Returns the message ID.
    pub async fn publish<T: Serialize>(&self, topic_name: &str, payload: &T) -> AppResult<String> {
        self.publish_with_options(topic_name, payload, None, None)
            .await
    }

    /// Publish with optional ordering key and attributes. Returns the message ID.
    pub async fn publish_with_options<T: Serialize>(
        &self,
        topic_name: &str,
        payload: &T,
        ordering_key: Option<&str>,
        attributes: Option<HashMap<String, String>>,
    ) -> AppResult<String> {
        let data = serde_json::to_vec(payload).map_err(|e| {
            AppError::PubsubPublishError(format!("Failed to serialize payload: {e}"))
        })?;

        let msg = PubsubMessage {
            data,
            attributes: attributes.unwrap_or_default(),
            ordering_key: ordering_key.unwrap_or_default().to_string(),
            ..Default::default()
        };

        let topic = self.client.topic(topic_name);
        let publisher = topic.new_publisher(None);

        let message_ids = publisher
            .publish_immediately(vec![msg], None)
            .await
            .map_err(|e| {
                AppError::PubsubPublishError(format!(
                    "Failed to publish to topic '{topic_name}': {e}"
                ))
            })?;

        let message_id = message_ids.into_iter().next().unwrap_or_default();

        debug!(
            topic = topic_name,
            message_id = %message_id,
            "Published message to Pub/Sub"
        );

        Ok(message_id)
    }

    pub async fn publish_timeslot_reserved(
        &self,
        event: TimeslotReservedEvent,
    ) -> AppResult<String> {
        self.publish("appointments", &event).await
    }

    pub async fn publish_timeslot_confirmed(
        &self,
        event: TimeslotConfirmedEvent,
    ) -> AppResult<String> {
        self.publish("appointments", &event).await
    }

    pub async fn publish_timeslot_released(
        &self,
        event: TimeslotReleasedEvent,
    ) -> AppResult<String> {
        self.publish("appointments", &event).await
    }
}
