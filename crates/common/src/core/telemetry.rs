use crate::config::TelemetryConfig;
use crate::core::GcpLogFormatter;
use anyhow::anyhow;
use opentelemetry::trace::TracerProvider as TracerProviderTrait;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use tracing_subscriber::Registry;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize OpenTelemetry tracing with the given configuration
///
/// Returns a TracerProvider that must be kept alive and shut down gracefully
/// to ensure all pending spans are exported.
pub fn init_telemetry(cfg: &TelemetryConfig) -> anyhow::Result<Option<SdkTracerProvider>> {
    // Initialize subscriber with both OpenTelemetry and GCP log formatting
    let is_gcp = std::env::var("LOG_STRUCTURE").ok().as_deref() == Some("googlecloud");

    if !cfg.enabled {
        // Initialize without OpenTelemetry layer
        if is_gcp {
            tracing_subscriber::fmt()
                .event_format(GcpLogFormatter::new())
                .with_env_filter(
                    EnvFilter::from_default_env()
                        .add_directive("server=info".parse()?)
                        .add_directive("tower_http=info".parse()?),
                )
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::from_default_env()
                        .add_directive("server=info".parse()?)
                        .add_directive("tower_http=info".parse()?),
                )
                .with_ansi(true)
                .init();
        }
        return Ok(None);
    }

    // Build resource with service metadata
    let resource = opentelemetry_sdk::Resource::builder()
        .with_attribute(opentelemetry::KeyValue::new(
            "service.name",
            cfg.service_name.clone(),
        ))
        .with_attribute(opentelemetry::KeyValue::new(
            "service.version",
            env!("CARGO_PKG_VERSION"),
        ))
        .build();

    // Create tracer provider with OTLP exporter
    let provider = if let Some(endpoint) = &cfg.exporter_otlp_endpoint {
        // Use OTLP gRPC exporter (for production with OpenTelemetry Collector)
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .map_err(|e| anyhow!("Failed to create OTLP exporter: {}", e))?;

        // Create tracer provider with batch exporter
        SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .with_sampler(Sampler::AlwaysOn)
            .build()
    } else {
        // For local development, use stdout logging only
        tracing::warn!(
            "No TELEMETRY__EXPORTER_OTLP_ENDPOINT configured, using stdout logging only"
        );
        // Initialize without OpenTelemetry layer
        if is_gcp {
            tracing_subscriber::fmt()
                .event_format(GcpLogFormatter::new())
                .with_env_filter(
                    EnvFilter::from_default_env()
                        .add_directive("server=info".parse()?)
                        .add_directive("tower_http=info".parse()?),
                )
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::from_default_env()
                        .add_directive("server=info".parse()?)
                        .add_directive("tower_http=info".parse()?),
                )
                .with_ansi(true)
                .init();
        }
        return Ok(None);
    };

    // Create OpenTelemetry tracing layer
    let telemetry = tracing_opentelemetry::layer().with_tracer(provider.tracer("doctor-app"));

    if is_gcp {
        Registry::default()
            .with(telemetry)
            .with(
                tracing_subscriber::fmt::layer()
                    .event_format(GcpLogFormatter::new())
                    .with_filter(
                        EnvFilter::from_default_env()
                            .add_directive("server=info".parse()?)
                            .add_directive("tower_http=info".parse()?),
                    ),
            )
            .init();
    } else {
        Registry::default()
            .with(telemetry)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(true)
                    .with_filter(
                        EnvFilter::from_default_env()
                            .add_directive("server=info".parse()?)
                            .add_directive("tower_http=info".parse()?),
                    ),
            )
            .init();
    }

    tracing::info!(
        service_name = %cfg.service_name,
        version = %env!("CARGO_PKG_VERSION"),
        "OpenTelemetry initialized with OTLP exporter"
    );

    Ok(Some(provider))
}

/// Shutdown the tracer provider gracefully
///
/// This ensures all pending spans are flushed before the application exits.
pub async fn shutdown_telemetry(provider: Option<SdkTracerProvider>) {
    if let Some(p) = provider {
        tracing::info!("Shutting down OpenTelemetry tracer provider...");
        if let Err(e) = p.shutdown() {
            tracing::error!("Failed to shutdown tracer provider: {}", e);
        } else {
            tracing::info!("OpenTelemetry tracer provider shutdown complete");
        }
    }
}
