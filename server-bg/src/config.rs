use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct HttpServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceConfig {
    pub iam_gatekeeper_base_uri: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirestoreCollections {
    pub notifications: String,
    pub fcm_tokens: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirestoreConfig {
    pub gcp_project_id: String,
    pub database_id: String,
    pub collections: FirestoreCollections,
}

impl FirestoreConfig {
    pub fn to_common(&self) -> common::config::FirestoreConfig {
        common::config::FirestoreConfig {
            gcp_project_id: self.gcp_project_id.clone(),
            database_id: self.database_id.clone(),
            collections: common::config::FirestoreCollections {
                appointments: String::new(),
                notifications: self.collections.notifications.clone(),
                doctors: String::new(),
                onboardings: String::new(),
                fcm_tokens: self.collections.fcm_tokens.clone(),
            },
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirebaseCollections {
    pub appointments: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirebaseConfig {
    pub database_url: String,
    pub collections: FirebaseCollections,
}

impl FirebaseConfig {
    pub fn to_common(&self) -> common::config::FirebaseConfig {
        common::config::FirebaseConfig {
            database_url: self.database_url.clone(),
            collections: common::config::FirebaseCollections {
                appointments: self.collections.appointments.clone(),
            },
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct FcmConfig {
    pub project_id: String,
    pub api_v1: String,
    #[serde(default)]
    pub circuit_breaker: FcmCircuitBreakerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FcmCircuitBreakerConfig {
    #[serde(default = "default_cb_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_cb_success_threshold")]
    pub success_threshold: u32,
    #[serde(default = "default_cb_open_timeout_secs")]
    pub open_timeout_secs: u64,
    #[serde(default = "default_cb_enabled")]
    pub enabled: bool,
}

impl Default for FcmCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_cb_failure_threshold(),
            success_threshold: default_cb_success_threshold(),
            open_timeout_secs: default_cb_open_timeout_secs(),
            enabled: default_cb_enabled(),
        }
    }
}

fn default_cb_failure_threshold() -> u32 {
    5
}

fn default_cb_success_threshold() -> u32 {
    2
}

fn default_cb_open_timeout_secs() -> u64 {
    60
}

fn default_cb_enabled() -> bool {
    false
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelemetryConfig {
    #[serde(default = "default_service_name")]
    pub service_name: String,
    pub exporter_otlp_endpoint: Option<String>,
    #[serde(default = "default_telemetry_enabled")]
    pub enabled: bool,
}

impl TelemetryConfig {
    pub fn to_common(&self) -> common::config::TelemetryConfig {
        common::config::TelemetryConfig {
            service_name: self.service_name.clone(),
            exporter_otlp_endpoint: self.exporter_otlp_endpoint.clone(),
            enabled: self.enabled,
        }
    }
}

fn default_service_name() -> String {
    "doctor-app-bg".to_string()
}

fn default_telemetry_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubsubTopics {
    pub doctor_notifications: String,
    pub doctor_profile_approved: String,
    pub doctor_profile_status_updated: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubsubConfig {
    pub gcp_project_id: String,
    pub emulator_host: Option<String>,
    pub topics: PubsubTopics,
}

impl PubsubConfig {
    pub fn to_common(&self) -> common::config::PubsubConfig {
        common::config::PubsubConfig {
            gcp_project_id: self.gcp_project_id.clone(),
            emulator_host: self.emulator_host.clone(),
            topics: common::config::PubsubTopics {
                appointments: String::new(),
                consultations: String::new(),
                system: String::new(),
                broadcast: String::new(),
                doctor_notifications: self.topics.doctor_notifications.clone(),
                doctor_profile_approved: self.topics.doctor_profile_approved.clone(),
                doctor_profile_status_updated: self.topics.doctor_profile_status_updated.clone(),
            },
            subscriptions: common::config::PubsubSubscriptions {
                notification: String::new(),
                consultation: String::new(),
                doctor_calendar_update: None,
                doctor_notification_send: None,
            },
            subscriber: common::config::PubsubSubscriberConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DoctorProfileOutboxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_outbox_batch_size")]
    pub batch_size: i64,
    #[serde(default = "default_outbox_lease_seconds")]
    pub lease_seconds: i64,
    #[serde(default = "default_outbox_poll_seconds")]
    pub poll_seconds: u64,
    #[serde(default = "default_outbox_publish_timeout_seconds")]
    pub publish_timeout_seconds: u64,
}

impl Default for DoctorProfileOutboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            batch_size: default_outbox_batch_size(),
            lease_seconds: default_outbox_lease_seconds(),
            poll_seconds: default_outbox_poll_seconds(),
            publish_timeout_seconds: default_outbox_publish_timeout_seconds(),
        }
    }
}

impl DoctorProfileOutboxConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.enabled {
            return Ok(());
        }
        if !(1..=100).contains(&self.batch_size)
            || self.lease_seconds < 2
            || self.poll_seconds == 0
            || self.publish_timeout_seconds == 0
        {
            return Err(ConfigError::Message("doctor_profile_outbox requires batch_size 1..=100 and positive lease/poll/publish timeout values".into()));
        }
        // The relay publishes sequentially. Keep the entire bounded batch inside
        // its lease rather than allowing a slow Pub/Sub call to duplicate it.
        if self
            .publish_timeout_seconds
            .saturating_mul(self.batch_size as u64)
            >= self.lease_seconds as u64
        {
            return Err(ConfigError::Message("doctor_profile_outbox lease_seconds must exceed batch_size * publish_timeout_seconds".into()));
        }
        Ok(())
    }
}
fn default_outbox_batch_size() -> i64 {
    10
}
fn default_outbox_lease_seconds() -> i64 {
    60
}
fn default_outbox_poll_seconds() -> u64 {
    1
}
fn default_outbox_publish_timeout_seconds() -> u64 {
    5
}

#[derive(Debug, Deserialize, Clone)]
pub struct CloudTasksConfig {
    pub gcp_project_id: String,
    pub gcp_location: String,
    pub queue_name: String,
    pub emulator_host: Option<String>,
    pub handler_base_url: String,
    #[serde(default)]
    pub oidc_service_account_email: String,
    #[serde(default)]
    pub oidc_audience: Option<String>,
}

impl CloudTasksConfig {
    pub fn to_common(&self) -> common::config::CloudTasksConfig {
        common::config::CloudTasksConfig {
            gcp_project_id: self.gcp_project_id.clone(),
            gcp_location: self.gcp_location.clone(),
            queue_name: self.queue_name.clone(),
            emulator_host: self.emulator_host.clone(),
            handler_base_url: self.handler_base_url.clone(),
            oidc_service_account_email: self.oidc_service_account_email.clone(),
            oidc_audience: self.oidc_audience.clone(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RetryConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_base_delay_ms")]
    pub base_delay_ms: u64,
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,
    #[serde(default = "default_max_elapsed_time_ms")]
    pub max_elapsed_time_ms: u64,
    #[serde(default = "default_jitter_ms")]
    pub jitter_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            base_delay_ms: default_base_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            max_elapsed_time_ms: default_max_elapsed_time_ms(),
            jitter_ms: default_jitter_ms(),
        }
    }
}

fn default_max_retries() -> u32 {
    3
}

fn default_base_delay_ms() -> u64 {
    1000
}

fn default_max_delay_ms() -> u64 {
    10000
}

fn default_max_elapsed_time_ms() -> u64 {
    30000
}

fn default_jitter_ms() -> u64 {
    500
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub http_server: HttpServerConfig,
    pub service: ServiceConfig,
    pub firestore: FirestoreConfig,
    pub firebase: FirebaseConfig,
    pub fcm: FcmConfig,
    pub telemetry: TelemetryConfig,
    pub pubsub: PubsubConfig,
    pub cloud_tasks: CloudTasksConfig,
    #[serde(default)]
    pub doctor_profile_outbox: DoctorProfileOutboxConfig,
    pub postgres: Option<common::config::PostgresConfig>,
    #[serde(default)]
    pub retry: RetryConfig,
}

impl AppConfig {
    pub fn load_from_dir(config_dir: Option<String>) -> Result<Self, ConfigError> {
        let config_path = match config_dir {
            Some(dir) => std::path::PathBuf::from(dir),
            None => std::path::PathBuf::from("./config"),
        };

        let config: Self = Config::builder()
            .add_source(File::from(config_path.join("default.toml")).required(true))
            .add_source(File::from(config_path.join("local.toml")).required(false))
            .add_source(Environment::default().separator("__").try_parsing(true))
            .build()?
            .try_deserialize()?;
        config.doctor_profile_outbox.validate()?;
        if config.doctor_profile_outbox.enabled && config.postgres.is_none() {
            return Err(ConfigError::Message(
                "postgres configuration is required when doctor_profile_outbox is enabled".into(),
            ));
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_config_loads_with_server_bg_runtime_fields_only() {
        let config_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
        let cfg =
            AppConfig::load_from_dir(Some(config_dir.to_string_lossy().into_owned())).unwrap();

        assert_eq!(cfg.service.iam_gatekeeper_base_uri, "http://localhost:9101");
        assert_eq!(cfg.firestore.collections.notifications, "notifications");
        assert_eq!(
            cfg.pubsub.topics.doctor_notifications,
            "doctor-notification-topic-v1"
        );
    }
}
