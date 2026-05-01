mod cli;
mod dispatch;
mod handlers;
mod obs;
mod output;
mod preset;

use forgeiso_engine::ForgeIsoEngine;

// Re-exported at the crate root because the handlers in `handlers::*` reference
// these names via `crate::*` (the previous monolithic main.rs declared them at
// the crate root).
pub(crate) use cli::{SourcesCmd, VmCmd};
pub(crate) use preset::{parse_profile, resolve_source_from_preset_or_str};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // JSON tracing — fail-open. Guard held for program lifetime.
    let _tracing_guard = obs::init_tracing();

    // OpenTelemetry tracing — feature-gated. Guard held for program lifetime
    // so the exporter flushes on Drop. With the `otel` feature off, this is a
    // zero-cost no-op guard.
    #[cfg(feature = "otel")]
    let _otel = forgeiso_engine::observability::init_otel(
        std::env::var("FORGEISO_OTEL_ENDPOINT").ok().as_deref(),
    );

    let parsed = <cli::Cli as clap::Parser>::parse();
    let engine = ForgeIsoEngine::new();

    // Subscribe to engine events and spawn event handler
    let _event_task = output::spawn_event_subscriber(&engine);
    // Parallel structured-log channel — does not replace user-facing stderr.
    let _trace_task = obs::spawn_event_tracer(&engine);

    dispatch::run(&engine, parsed.command).await
}
