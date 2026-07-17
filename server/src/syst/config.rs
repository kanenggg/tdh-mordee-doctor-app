use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceConfig {
    pub biz_jade_service_base_uri: String,
    pub privilege_service_base_uri: String,
    pub consultation_base_uri: String,
    pub iam_gatekeeper_base_uri: String,
    pub payment_internal_base_uri: String,
    pub eagle_base_uri: String,
    /// Base URI for the QOLPHIN past-visit service.
    /// Override with `SERVICE__QOLPHIN_BASE_URI` env var.
    pub qolphin_base_uri: String,
    /// Base URI for the biz-apm v2 service (used by ehr past-visit-detail).
    /// Override with `SERVICE__BIZ_APM_BASE_URI` env var.
    pub biz_apm_base_uri: String,
    /// Keep origin/main Pub/Sub delivery enabled while the durable relay rolls out.
    #[serde(default = "default_doctor_profile_immediate_delivery_enabled")]
    pub doctor_profile_immediate_delivery_enabled: bool,
    /// Base URI for the EHR upstream (lab-result, past-visit).
    /// Override with `SERVICE__EHR_SERVICE_BASE_URI` env var.
    pub ehr_service_base_uri: String,
}

fn default_doctor_profile_immediate_delivery_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct RankingConfig {
    #[serde(default = "default_platform_fee_multiplier")]
    pub platform_fee_multiplier: f64,
}

fn default_platform_fee_multiplier() -> f64 {
    1.5
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirestoreCollections {
    pub notifications: String,
    #[allow(dead_code)]
    pub doctors: String,
    pub fcm_tokens: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirestoreConfig {
    pub gcp_project_id: String,
    pub database_id: String,
    pub collections: FirestoreCollections,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FirebaseConfig {
    pub database_url: String,
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
    false // Disabled by default for safety
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubsubSubscriptions {
    pub notification: String,
    pub consultation: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubsubSubscriberConfig {
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_subscriber_enabled")]
    pub enabled: bool,
}

impl Default for PubsubSubscriberConfig {
    fn default() -> Self {
        Self {
            max_concurrency: default_max_concurrency(),
            enabled: default_subscriber_enabled(),
        }
    }
}

fn default_max_concurrency() -> usize {
    10
}

fn default_subscriber_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelemetryConfig {
    #[serde(default = "default_service_name")]
    pub service_name: String,
    pub exporter_otlp_endpoint: Option<String>,
    #[serde(default = "default_telemetry_enabled")]
    pub enabled: bool,
}

fn default_service_name() -> String {
    "doctor-app".to_string()
}

fn default_telemetry_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubsubTopics {
    pub appointments: String,
    pub consultations: String,
    pub system: String,
    pub broadcast: String,
    pub doctor_notifications: String,
    pub doctor_profile_approved: String,
    pub doctor_profile_status_updated: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PubsubConfig {
    pub gcp_project_id: String,
    pub emulator_host: Option<String>,
    pub topics: PubsubTopics,
    pub subscriptions: PubsubSubscriptions,
    #[serde(default)]
    pub subscriber: PubsubSubscriberConfig,
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
    30000 // 30 seconds
}

fn default_jitter_ms() -> u64 {
    500 // 0.5 seconds
}

#[derive(Debug, Deserialize, Clone)]
pub struct GcsConfig {
    pub bucket_name: String,
    pub base_url: String,
    #[serde(default)]
    pub signing_service_account_email: String,
    #[serde(default = "default_gcs_signed_url_ttl_secs")]
    pub signed_url_ttl_secs: u32,
}

fn default_gcs_signed_url_ttl_secs() -> u32 {
    300
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostgresConfig {
    pub host: String,
    #[serde(default = "default_postgres_port")]
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
}

impl PostgresConfig {
    pub fn database_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.dbname
        )
    }
}

fn default_postgres_port() -> u16 {
    5432
}

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    #[serde(default = "default_daily_limit")]
    pub daily_limit: i32,
    #[serde(default = "default_weekly_limit")]
    pub weekly_limit: i32,
    #[serde(default = "default_ratelimit_enabled")]
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            daily_limit: default_daily_limit(),
            weekly_limit: default_weekly_limit(),
            enabled: default_ratelimit_enabled(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct TimeslotConfig {
    #[serde(default = "default_max_query_range_days")]
    pub max_query_range_days: i32,
    #[serde(default = "default_reservation_ttl")]
    pub default_reservation_ttl: i32,
    #[serde(default = "default_min_reservation_ttl")]
    pub min_reservation_ttl: i32,
    #[serde(default = "default_max_reservation_ttl")]
    pub max_reservation_ttl: i32,
    #[serde(default = "default_expiry_check_interval")]
    pub expiry_check_interval: u64,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

impl Default for TimeslotConfig {
    fn default() -> Self {
        Self {
            max_query_range_days: default_max_query_range_days(),
            default_reservation_ttl: default_reservation_ttl(),
            min_reservation_ttl: default_min_reservation_ttl(),
            max_reservation_ttl: default_max_reservation_ttl(),
            expiry_check_interval: default_expiry_check_interval(),
            rate_limit: RateLimitConfig::default(),
        }
    }
}

fn default_max_query_range_days() -> i32 {
    30
}

fn default_reservation_ttl() -> i32 {
    300
}

fn default_min_reservation_ttl() -> i32 {
    60
}

fn default_max_reservation_ttl() -> i32 {
    3600
}

fn default_expiry_check_interval() -> u64 {
    60
}

fn default_daily_limit() -> i32 {
    10
}

fn default_weekly_limit() -> i32 {
    50
}

fn default_ratelimit_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PasetoConfig {
    /// 32-byte hex-encoded symmetric key for Paseto v4 local encryption.
    /// Override with PASETO__LOCAL_KEY .env var in production.
    pub summarization_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KmsConfig {
    /// GCP KMS crypto key resource:
    /// `projects/{p}/locations/{l}/keyRings/{r}/cryptoKeys/{k}`.
    /// Empty means KMS is unconfigured (encrypt/decrypt calls then error).
    #[serde(default)]
    pub doctor_profile_pii_data: String,
}

impl Default for KmsConfig {
    fn default() -> Self {
        Self {
            doctor_profile_pii_data: String::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct InsuranceConfig {
    /// URL template for the insurance terms-and-conditions HTML page.
    /// Must contain a single `{privilegeId}` placeholder.
    /// e.g. "https://storage.googleapis.com/truehealth-public/instructions/insurance/{privilegeId}.html"
    pub condition_url_template: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CouponConfig {
    /// URL template for the coupon terms-and-conditions HTML page.
    /// Must contain a single `{couponKey}` placeholder.
    /// e.g. "https://static.tdh.com/coupon/{couponKey}.html"
    pub condition_url_template: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub http_server: HttpServerConfig,
    pub service: ServiceConfig,
    pub firestore: FirestoreConfig,
    pub firebase: FirebaseConfig,
    pub fcm: FcmConfig,
    #[allow(dead_code)]
    pub telemetry: TelemetryConfig,
    pub pubsub: PubsubConfig,
    pub gcs: GcsConfig,
    pub retry: RetryConfig,
    pub postgres: PostgresConfig,
    pub ranking: RankingConfig,
    pub redis: RedisConfig,
    pub paseto: PasetoConfig,
    #[serde(default)]
    pub kms: KmsConfig,
    #[serde(default)]
    pub timeslot: TimeslotConfig,
    /// Required. Startup deserialization fails if the `[insurance]`
    /// block is missing. Validated at startup by `module::appointment::router`.
    pub insurance: InsuranceConfig,
    /// Required. Startup deserialization fails if the `[coupon]` block
    /// is missing. Validated at startup by `module::appointment::router`.
    pub coupon: CouponConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_dir(None)
    }

    pub fn load_from_dir(config_dir: Option<String>) -> Result<Self, ConfigError> {
        let config_path = match config_dir {
            Some(dir) => std::path::PathBuf::from(dir),
            None => {
                // Default to current directory
                std::path::PathBuf::from("./config")
            }
        };

        Config::builder()
            .add_source(File::from(config_path.join("default.toml")).required(true))
            .add_source(File::from(config_path.join("local.toml")).required(false))
            // .add_source(
            //     Environment::default()
            //         .separator("__")
            //         .try_parsing(true)
            //         .with_list_parse_key("sys.host")
            //         .convert_case(config::Case::Snake),
            // )
            // Support simple env var overrides matching the Scala app
            // Telemetry config with environment defaults
            // .set_override_option(
            //     "telemetry.service_name",
            //     std::env::var("TELEMETRY__SERVICE_NAME").ok(),
            // )?
            // .set_override_option(
            //     "telemetry.exporter_otlp_endpoint",
            //     std::env::var("TELEMETRY__EXPORTER_OTLP_ENDPOINT").ok(),
            // )?
            // .set_override_option(
            //     "telemetry.enabled",
            //     std::env::var("TELEMETRY__ENABLED")
            //         .ok()
            //         .and_then(|v| v.parse::<bool>().ok())
            //         .map(|b| b.to_string()),
            // )?
            .add_source(Environment::default().separator("__").try_parsing(true))
            .build()?
            .try_deserialize()
    }
}
