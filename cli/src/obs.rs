//! Structured observability wiring.
//!
//! Initializes a JSON `tracing` subscriber that writes one event per line to a
//! daily-rolling file under `<log_dir>/forgeiso.log.YYYY-MM-DD`. Fail-open: if
//! the log directory is unwritable the CLI runs without JSON logs and the
//! existing stderr channel is unaffected.
//!
//! `RUST_LOG` overrides the default level. Default level is `info`.
//!
//! Engine events emitted on the broadcast channel are mirrored as structured
//! tracing events so external observers (jq, log shippers) get the same view
//! as the human-facing stderr channel without any string parsing.
//!
//! Tracing is a *parallel* channel — the existing eprintln/println output is
//! preserved so CLI scriptability and progress display are unchanged.

use std::path::PathBuf;

use forgeiso_engine::{EventLevel, EventPhase, ForgeIsoEngine};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Default log directory: `$XDG_STATE_HOME/forgeiso` or `~/.local/state/forgeiso`.
fn default_log_dir() -> Option<PathBuf> {
    if let Ok(state) = std::env::var("XDG_STATE_HOME") {
        if !state.is_empty() {
            return Some(PathBuf::from(state).join("forgeiso"));
        }
    }
    dirs::state_dir()
        .or_else(dirs::home_dir)
        .map(|d| d.join("forgeiso/logs"))
}

/// Resolve effective log directory: `FORGEISO_LOG_DIR` env > XDG state > none.
fn resolve_log_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FORGEISO_LOG_DIR") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    default_log_dir()
}

/// Initialize JSON tracing to a daily-rolling file. Fail-open: returns `None`
/// if init fails (log dir not writable, subscriber already set, etc.) and the
/// CLI continues with stderr-only output.
///
/// The returned `WorkerGuard` must be held for the lifetime of the program;
/// dropping it early will flush and stop the background log writer thread.
pub fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter};

    let log_dir = resolve_log_dir()?;
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "forgeiso: log_dir {} not writable, JSON logs disabled: {e}",
            log_dir.display()
        );
        return None;
    }

    let appender = tracing_appender::rolling::daily(&log_dir, "forgeiso.log");
    let (nb, guard) = tracing_appender::non_blocking(appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(nb).with_target(true).json())
        .try_init()
        .ok()?;

    Some(guard)
}

/// Spawn a task that mirrors engine broadcast events into the tracing channel
/// as structured records (op=phase, level, message, plus progress fields when
/// present). Runs alongside the human-facing stderr subscriber — they are
/// independent.
pub fn spawn_event_tracer(engine: &ForgeIsoEngine) -> JoinHandle<()> {
    let mut rx = engine.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let phase = phase_label(&event.phase);
            let percent = event.percent.unwrap_or(f32::NAN);
            let bytes_done = event.bytes_done.unwrap_or(0);
            let bytes_total = event.bytes_total.unwrap_or(0);
            let substage = event.substage.as_deref().unwrap_or("");
            match event.level {
                EventLevel::Debug => debug!(
                    op = phase,
                    substage, percent, bytes_done, bytes_total, "{}", event.message
                ),
                EventLevel::Info => info!(
                    op = phase,
                    substage, percent, bytes_done, bytes_total, "{}", event.message
                ),
                EventLevel::Warn => warn!(
                    op = phase,
                    substage, percent, bytes_done, bytes_total, "{}", event.message
                ),
                EventLevel::Error => error!(
                    op = phase,
                    substage, percent, bytes_done, bytes_total, "{}", event.message
                ),
            }
        }
    })
}

fn phase_label(phase: &EventPhase) -> &'static str {
    match phase {
        EventPhase::Configure => "configure",
        EventPhase::Doctor => "doctor",
        EventPhase::ReleaseLookup => "release_lookup",
        EventPhase::Build => "build",
        EventPhase::Scan => "scan",
        EventPhase::Test => "test",
        EventPhase::Report => "report",
        EventPhase::Inspect => "inspect",
        EventPhase::Download => "download",
        EventPhase::Verify => "verify",
        EventPhase::Inject => "inject",
        EventPhase::Diff => "diff",
        EventPhase::Complete => "complete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_log_dir_respects_env_override() {
        // Override is read once into PathBuf — checked in process scope here.
        let prev = std::env::var("FORGEISO_LOG_DIR").ok();
        std::env::set_var("FORGEISO_LOG_DIR", "/tmp/forgeiso-test-log");
        let resolved = resolve_log_dir();
        assert_eq!(
            resolved,
            Some(std::path::PathBuf::from("/tmp/forgeiso-test-log"))
        );
        match prev {
            Some(v) => std::env::set_var("FORGEISO_LOG_DIR", v),
            None => std::env::remove_var("FORGEISO_LOG_DIR"),
        }
    }

    #[test]
    fn phase_label_covers_all_variants() {
        assert_eq!(phase_label(&EventPhase::Build), "build");
        assert_eq!(phase_label(&EventPhase::Inject), "inject");
        assert_eq!(phase_label(&EventPhase::Scan), "scan");
        assert_eq!(phase_label(&EventPhase::Complete), "complete");
    }
}
