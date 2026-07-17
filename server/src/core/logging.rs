//! GCP Cloud Logging Structured Logging Middleware
//!
//! Outputs structured JSON logs compatible with Google Cloud Logging
//! for Cloud Run deployments.
//! <https://cloud.google.com/logging/docs/structured-logging>
//!
//! Features:
//! - GCP Cloud Logging structured JSON format with `severity`, `message`, `time`
//! - HTTP request/response logging with `httpRequest` field
//! - Cloud Trace correlation via `X-Cloud-Trace-Context` header
//! - `logging.googleapis.com/labels` for custom metadata (request_id, security events)
//! - `logging.googleapis.com/sourceLocation` for application-level tracing logs
//! - Sensitive data masking (emails, phone numbers, OTP codes)

use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use serde::Serialize;
use std::{collections::HashMap, time::Instant};
use tracing::field::{Field, Visit};
use uuid::Uuid;

/// Newtype carrying the resolved request ID through Axum extensions.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

// ─── GCP Structured Log Types ───────────────────────────────────────────────

/// GCP `httpRequest` structured field.
/// <https://cloud.google.com/logging/docs/reference/v2/rest/v2/LogEntry#HttpRequest>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GcpHttpRequest {
    request_method: String,
    request_url: String,
    status: u16,
    user_agent: String,
    remote_ip: String,
    /// Duration in seconds with nanosecond precision, e.g. "0.012345678s"
    latency: String,
    protocol: String,
}

/// GCP structured log entry written to stdout as a single JSON line.
/// Cloud Run automatically ingests these into Cloud Logging.
#[derive(Debug, Clone, Serialize)]
struct GcpLogEntry {
    severity: String,
    message: String,
    time: String,

    #[serde(rename = "httpRequest", skip_serializing_if = "Option::is_none")]
    http_request: Option<GcpHttpRequest>,

    #[serde(rename = "logging.googleapis.com/insertId")]
    insert_id: String,

    #[serde(
        rename = "logging.googleapis.com/trace",
        skip_serializing_if = "Option::is_none"
    )]
    trace: Option<String>,

    #[serde(
        rename = "logging.googleapis.com/spanId",
        skip_serializing_if = "Option::is_none"
    )]
    span_id: Option<String>,

    #[serde(
        rename = "logging.googleapis.com/labels",
        skip_serializing_if = "Option::is_none"
    )]
    labels: Option<HashMap<String, String>>,
}

impl GcpLogEntry {
    fn new(severity: &str, message: String) -> Self {
        Self {
            severity: severity.to_string(),
            message,
            time: jiff::Zoned::now()
                .strftime("%Y-%m-%dT%H:%M:%S%.fZ")
                .to_string(),
            http_request: None,
            insert_id: Uuid::new_v4().to_string(),
            trace: None,
            span_id: None,
            labels: None,
        }
    }

    /// Write the log entry as a single JSON line to stdout.
    /// Cloud Run forwards stdout to Cloud Logging automatically.
    fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            println!("{}", json);
        }
    }
}

// ─── GCP-Compatible Tracing Formatter ───────────────────────────────────────
//
// This formatter is used by `tracing_subscriber` so that all application-level
// logs (info!, warn!, error!, debug!) also appear as GCP-structured JSON.

/// Visitor that extracts fields from a `tracing::Event`.
#[derive(Default)]
struct GcpEventVisitor {
    message: String,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl Visit for GcpEventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let val_str = format!("{:?}", value);
        if field.name() == "message" {
            self.message = val_str;
        } else {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::String(val_str));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::json!(value));
    }
}

/// GCP-compatible log formatter for `tracing_subscriber`.
///
/// Outputs each tracing event as a single-line JSON object with:
/// - `severity` – mapped from tracing level (DEBUG, INFO, WARNING, ERROR)
/// - `message` – the log message
/// - `time` – RFC 3339 timestamp with millisecond precision
/// - `logging.googleapis.com/sourceLocation` – file, line, module path
///
/// Use with `tracing_subscriber::fmt::layer().event_format(GcpLogFormatter::new())`.
pub struct GcpLogFormatter;

impl GcpLogFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GcpLogFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for GcpLogFormatter
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    N: for<'writer> tracing_subscriber::fmt::FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();

        // Map tracing levels to GCP Cloud Logging severity
        let severity = match *metadata.level() {
            tracing::Level::TRACE => "DEBUG",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::INFO => "INFO",
            tracing::Level::WARN => "WARNING",
            tracing::Level::ERROR => "ERROR",
        };

        let mut visitor = GcpEventVisitor::default();
        event.record(&mut visitor);

        let time = jiff::Zoned::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.fZ")
            .to_string();

        let mut json_map = serde_json::Map::new();
        json_map.insert(
            "severity".into(),
            serde_json::Value::String(severity.into()),
        );
        json_map.insert("message".into(), serde_json::Value::String(visitor.message));
        json_map.insert("time".into(), serde_json::Value::String(time));

        // logging.googleapis.com/sourceLocation
        if let (Some(file), Some(line)) = (metadata.file(), metadata.line()) {
            let mut source_loc = serde_json::Map::new();
            source_loc.insert("file".into(), serde_json::Value::String(file.into()));
            source_loc.insert("line".into(), serde_json::Value::String(line.to_string()));
            if let Some(module) = metadata.module_path() {
                source_loc.insert("function".into(), serde_json::Value::String(module.into()));
            }
            json_map.insert(
                "logging.googleapis.com/sourceLocation".into(),
                serde_json::Value::Object(source_loc),
            );
        }

        for (key, value) in visitor.fields {
            json_map.insert(key, value);
        }

        let json_str = serde_json::to_string(&serde_json::Value::Object(json_map))
            .map_err(|_| std::fmt::Error)?;
        writeln!(writer, "{}", json_str)
    }
}

// ─── Cloud Trace Context ────────────────────────────────────────────────────

/// Parse the `X-Cloud-Trace-Context` header injected by Cloud Run.
///
/// Format: `TRACE_ID/SPAN_ID;o=TRACE_TRUE`
/// e.g. `105445aa7843bc8bf206b12000100000/1;o=1`
///
/// Returns `(trace, span_id)` where trace is formatted as
/// `projects/{project_id}/traces/{trace_id}` for Cloud Logging correlation.
fn parse_cloud_trace_context(
    header_value: &str,
    project_id: &str,
) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = header_value.split('/').collect();
    if parts.is_empty() {
        return (None, None);
    }

    let trace_id = parts[0];
    let trace = if !trace_id.is_empty() && !project_id.is_empty() {
        Some(format!("projects/{}/traces/{}", project_id, trace_id))
    } else {
        None
    };

    let span_id = if parts.len() > 1 {
        parts[1]
            .split(';')
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    } else {
        None
    };

    (trace, span_id)
}

// ─── Request Metadata ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct RequestMetadata {
    request_id: String,
    method: String,
    path: String,
    query_string: String,
    user_agent: String,
    client_ip: String,
    protocol: String,
    trace: Option<String>,
    span_id: Option<String>,
}

/// Generate a request ID with format `yyMMdd` + 8 random uppercase alphanumeric chars.
/// Uses UUID v4 hex bytes as the random source (no extra dependency needed).
fn generate_request_id() -> String {
    let date = jiff::Zoned::now().strftime("%y%m%d").to_string();
    let uuid_hex = uuid::Uuid::new_v4().simple().to_string();
    let suffix: String = uuid_hex
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    format!("{}{}", date, suffix)
}

fn extract_request_metadata(req: &Request<Body>) -> RequestMetadata {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let query_string = req.uri().query().unwrap_or("").to_string();
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Prefer X-Forwarded-For (set by Cloud Run / load balancers), fall back to
    // the raw peer address stored by axum's ConnectInfo extension.
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.extensions()
                .get::<axum::extract::connect_info::ConnectInfo<std::net::SocketAddr>>()
                .map(|ci| ci.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let protocol = format!("{:?}", req.version());

    // Use caller-supplied request ID if present, otherwise generate one.
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(generate_request_id);

    let project_id = std::env::var("GOOGLE_CLOUD_PROJECT").unwrap_or_default();
    let (trace, span_id) = req
        .headers()
        .get("x-cloud-trace-context")
        .and_then(|h| h.to_str().ok())
        .map(|header| parse_cloud_trace_context(header, &project_id))
        .unwrap_or((None, None));

    RequestMetadata {
        request_id,
        method,
        path,
        query_string,
        user_agent,
        client_ip,
        protocol,
        trace,
        span_id,
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Map HTTP status code to GCP Cloud Logging severity level.
fn severity_from_status(status: u16) -> &'static str {
    match status {
        200..=299 => "INFO",
        300..=399 => "INFO",
        400..=499 => "WARNING",
        500..=599 => "ERROR",
        _ => "DEFAULT",
    }
}

pub(crate) fn mask_sensitive_data(data: &str) -> String {
    let national_id_pattern = regex::Regex::new(r"\b\d{13}\b").unwrap();
    let email_pattern =
        regex::Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap();
    let phone_pattern = regex::Regex::new(r"\+?\d{10,15}").unwrap();
    let otp_pattern = regex::Regex::new(r"\b\d{6}\b").unwrap();

    let mut masked = data.to_string();
    masked = national_id_pattern
        .replace_all(&masked, "***ID-MASKED***")
        .to_string();
    masked = email_pattern
        .replace_all(&masked, "***@***.***")
        .to_string();
    masked = phone_pattern
        .replace_all(&masked, "+***MASKED***")
        .to_string();
    masked = otp_pattern.replace_all(&masked, "******").to_string();
    masked
}

// ─── Axum Middleware ─────────────────────────────────────────────────────────

/// GCP Cloud Logging structured HTTP middleware for axum.
///
/// For each request/response pair, emits a single structured JSON log to
/// stdout with:
/// - `httpRequest`  – method, URL, status, latency, user agent, remote IP
/// - `severity`     – mapped from HTTP status (INFO / WARNING / ERROR)
/// - Cloud Trace    – correlated via `X-Cloud-Trace-Context` header
/// - `labels`       – request_id, security events (401/403/429), slow flag
///
/// Register with `axum::middleware::from_fn`:
/// ```ignore
/// Router::new().layer(axum::middleware::from_fn(gcp_logging_middleware))
/// ```
pub async fn gcp_logging_middleware(mut request: Request<Body>, next: Next) -> Response {
    let start_time = Instant::now();
    let metadata = extract_request_metadata(&request);

    // Inject request ID into extensions so handlers can extract it.
    request
        .extensions_mut()
        .insert(RequestId(metadata.request_id.clone()));

    let response = next.run(request).await;

    let duration = start_time.elapsed();
    let latency_secs = duration.as_secs_f64();
    let status_code = response.status().as_u16();
    let severity = severity_from_status(status_code);

    let request_url = if metadata.query_string.is_empty() {
        metadata.path.clone()
    } else {
        format!(
            "{}?{}",
            metadata.path,
            mask_sensitive_data(&metadata.query_string)
        )
    };

    let http_request = GcpHttpRequest {
        request_method: metadata.method.clone(),
        request_url,
        status: status_code,
        user_agent: metadata.user_agent.clone(),
        remote_ip: metadata.client_ip.clone(),
        latency: format!("{:.9}s", latency_secs),
        protocol: metadata.protocol.clone(),
    };

    let message = format!(
        "{} {} {} {:.2}ms",
        metadata.method,
        metadata.path,
        status_code,
        latency_secs * 1000.0,
    );

    let mut labels = HashMap::new();
    labels.insert("request_id".to_string(), metadata.request_id.clone());

    match status_code {
        401 => {
            labels.insert(
                "security_event".to_string(),
                "authentication_failed".to_string(),
            );
        }
        403 => {
            labels.insert(
                "security_event".to_string(),
                "authorization_failed".to_string(),
            );
        }
        429 => {
            labels.insert(
                "security_event".to_string(),
                "rate_limit_exceeded".to_string(),
            );
        }
        _ => {}
    }

    if duration.as_secs() >= 1 {
        labels.insert("slow_request".to_string(), "true".to_string());
    }

    let mut log_entry = GcpLogEntry::new(severity, message);
    log_entry.http_request = Some(http_request);
    log_entry.trace = metadata.trace;
    log_entry.span_id = metadata.span_id;
    log_entry.insert_id = metadata.request_id;
    log_entry.labels = Some(labels);
    log_entry.emit();

    response
}
