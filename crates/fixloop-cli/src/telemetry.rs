//! Logging and tracing setup: `tracing` to stderr (pretty or JSON) plus
//! optional OTLP trace export.

use fixloop_core::config::{LogFormat, TelemetryConfig};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::CliError;

/// Flushes and shuts down trace export when dropped.
pub(crate) struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take()
            && let Err(e) = provider.shutdown()
        {
            eprintln!("warning: could not flush traces: {e}");
        }
    }
}

/// Install the global subscriber. Must be called inside a Tokio runtime when
/// OTLP export is enabled.
pub(crate) fn init(cfg: &TelemetryConfig) -> Result<TelemetryGuard, CliError> {
    let filter = EnvFilter::try_new(&cfg.log_level).map_err(|e| {
        CliError::new(
            format!("invalid log level `{}`: {e}", cfg.log_level),
            Some("use a level like `info` or a filter like `fixloop=debug,info`".into()),
        )
    })?;

    let provider = match &cfg.otlp_endpoint {
        Some(endpoint) => {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint.as_str())
                .build()
                .map_err(|e| {
                    CliError::new(
                        format!("could not set up OTLP export to {endpoint}: {e}"),
                        Some("check telemetry.otlp_endpoint, or unset it to disable export".into()),
                    )
                })?;
            Some(
                SdkTracerProvider::builder()
                    .with_batch_exporter(exporter)
                    .with_resource(
                        Resource::builder()
                            .with_service_name(cfg.service_name.clone())
                            .build(),
                    )
                    .build(),
            )
        }
        None => None,
    };
    let otel_layer = provider
        .as_ref()
        .map(|p| tracing_opentelemetry::layer().with_tracer(p.tracer("fixloop")));

    let fmt_layer = match cfg.log_format {
        LogFormat::Pretty => tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .boxed(),
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_writer(std::io::stderr)
            .boxed(),
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init()
        .map_err(|e| CliError::new(format!("could not install logger: {e}"), None))?;

    Ok(TelemetryGuard { provider })
}
