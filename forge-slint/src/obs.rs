//! GUI observability — JSON tracing parallel to the in-app log pane.
//!
//! Fail-open: returns `None` if log dir unwritable; the GUI continues with
//! its on-screen log model unaffected. `RUST_LOG` overrides default level.

use std::path::PathBuf;

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

fn resolve_log_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FORGEISO_LOG_DIR") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    default_log_dir()
}

pub fn init_tracing() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter};

    let log_dir = resolve_log_dir()?;
    if std::fs::create_dir_all(&log_dir).is_err() {
        return None;
    }
    let appender = tracing_appender::rolling::daily(&log_dir, "forgeiso-gui.log");
    let (nb, guard) = tracing_appender::non_blocking(appender);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(nb).with_target(true).json())
        .try_init()
        .ok()?;
    Some(guard)
}
