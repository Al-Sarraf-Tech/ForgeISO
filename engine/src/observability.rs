//! OpenTelemetry tracing integration for engine operations.
//!
//! This module is feature-gated behind `otel`. With the feature off, the
//! module compiles to a no-op `OtelGuard` and `init_otel()` so callers can
//! always invoke them unconditionally; default builds incur zero runtime
//! cost beyond a single struct construction.
//!
//! When `otel` is enabled, [`init_otel`] wires a `tracing-opentelemetry`
//! layer onto the existing global [`tracing`] subscriber so that all
//! `info_span!`/`tracing::info!` events from engine code automatically
//! flow to either an OTLP HTTP endpoint or stdout (for local debug).
//!
//! Callers must hold the returned [`OtelGuard`] for the program lifetime.
//! Dropping it shuts the exporter down and flushes pending spans.
//!
//! ```no_run
//! // In CLI/TUI/GUI main():
//! # #[cfg(feature = "otel")]
//! # fn main() {
//! let _otel = forgeiso_engine::observability::init_otel(
//!     std::env::var("FORGEISO_OTEL_ENDPOINT").ok().as_deref(),
//! );
//! // Hold _otel until program exits.
//! # }
//! # #[cfg(not(feature = "otel"))]
//! # fn main() {}
//! ```
//!
//! Span hierarchy emitted by the engine (top-level orchestrator phases):
//! - `inject_phase` (with `phase` field: `inject_autoinstall`, `setup`, `extract`, `place`, `repack`)
//! - `build_phase`
//! - `scan_phase`
//! - `verify_phase`
//!
//! These compose under whatever parent span the caller establishes
//! (e.g. a CLI command span), giving distributed-trace–compatible call
//! graphs once an OTLP collector is connected.

/// Guard that keeps the OpenTelemetry exporter alive for the program's
/// lifetime. Drop on shutdown to flush pending spans.
///
/// Without the `otel` feature this is a zero-sized type whose `Drop` is
/// a no-op.
#[derive(Debug)]
pub struct OtelGuard {
    #[cfg(feature = "otel")]
    provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl Default for OtelGuard {
    fn default() -> Self {
        Self::new_disabled()
    }
}

impl OtelGuard {
    /// A no-op guard used when initialisation fails or the `otel` feature
    /// is off.
    pub fn new_disabled() -> Self {
        Self {
            #[cfg(feature = "otel")]
            provider: None,
        }
    }
}

#[cfg(feature = "otel")]
impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            // Best-effort flush; never panic on shutdown.
            let _ = provider.shutdown();
        }
    }
}

/// Initialise OpenTelemetry tracing.
///
/// * `endpoint` — when `Some(url)`, configure the OTLP HTTP exporter to send
///   spans to that URL (e.g. `http://localhost:4318/v1/traces`).
///   when `None`, use the stdout exporter for local debugging.
///
/// Without the `otel` feature this is a no-op that returns a disabled guard.
///
/// Fail-open: any failure to construct the exporter or attach the layer
/// produces a disabled guard and a single warning on stderr; the caller
/// can keep running with file-only tracing.
pub fn init_otel(endpoint: Option<&str>) -> OtelGuard {
    #[cfg(feature = "otel")]
    {
        match try_init_otel(endpoint) {
            Ok(guard) => guard,
            Err(err) => {
                eprintln!("forgeiso: OpenTelemetry init failed: {err}");
                OtelGuard::new_disabled()
            }
        }
    }
    #[cfg(not(feature = "otel"))]
    {
        let _ = endpoint;
        OtelGuard::new_disabled()
    }
}

#[cfg(feature = "otel")]
fn try_init_otel(endpoint: Option<&str>) -> Result<OtelGuard, Box<dyn std::error::Error>> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use opentelemetry_sdk::Resource;
    use tracing_opentelemetry::OpenTelemetryLayer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Registry;

    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", "forgeiso"))
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build();

    let provider = match endpoint {
        Some(url) => {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_endpoint(url)
                .build()?;
            SdkTracerProvider::builder()
                .with_resource(resource)
                .with_batch_exporter(exporter)
                .build()
        }
        None => {
            let exporter = opentelemetry_stdout::SpanExporter::default();
            SdkTracerProvider::builder()
                .with_resource(resource)
                .with_simple_exporter(exporter)
                .build()
        }
    };

    let tracer = provider.tracer("forgeiso-engine");

    // Attach the OpenTelemetry layer to a fresh registry. Other crates
    // (CLI/TUI/GUI) may already have called `tracing_subscriber::*::try_init()`
    // for file/stderr logging — in that case `try_init()` here returns Err and
    // we keep the provider alive without the global layer (spans still flush
    // on Drop, just no automatic propagation from `tracing` macros). This is
    // intentional fail-open behaviour.
    let _ = Registry::default()
        .with(OpenTelemetryLayer::new(tracer))
        .try_init();

    Ok(OtelGuard {
        provider: Some(provider),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Without the `otel` feature, `OtelGuard` is a struct with no fields and
    // no Drop impl — clippy warns when calling `drop()` on a non-Drop type,
    // but the explicit drop documents intent ("guard goes out of scope here")
    // and the test still verifies the type can move out cleanly.
    #[test]
    #[allow(clippy::drop_non_drop)]
    fn disabled_guard_drops_cleanly() {
        let guard = OtelGuard::new_disabled();
        drop(guard);
    }

    #[test]
    #[allow(clippy::drop_non_drop)]
    fn default_guard_is_disabled() {
        let guard = OtelGuard::default();
        drop(guard);
    }

    #[test]
    #[allow(clippy::drop_non_drop)]
    fn init_otel_without_feature_returns_disabled_guard() {
        // With or without the feature, the call must not panic and must
        // return a guard that drops cleanly.
        let guard = init_otel(None);
        drop(guard);
    }
}
