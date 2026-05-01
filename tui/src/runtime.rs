//! Runtime helpers used by `main()`: drain the async worker channel into
//! the [`App`] state on every tick.

use tokio::sync::mpsc;

use crate::state::{App, LogLevel, WorkerMsg};

/// Drain every pending worker message into `app`. Mirrors the original
/// inline `while let Ok(msg) = rx_worker.try_recv()` block in `main()`.
pub(crate) fn drain_worker(app: &mut App, rx: &mut mpsc::UnboundedReceiver<WorkerMsg>) {
    while let Ok(msg) = rx.try_recv() {
        match msg {
            WorkerMsg::EngineEvent(text, level) => {
                app.push_log(text, level);
            }
            WorkerMsg::InspectOk(info) => {
                app.busy = false;
                app.detected_distro = info
                    .distro
                    .map(|d| format!("{d:?}"))
                    .or_else(|| Some("Unknown".into()));
                app.status = "Source inspected".into();
            }
            WorkerMsg::InjectOk(result) => {
                app.busy = false;
                app.progress.build_done = true;
                app.build_artifact = result.artifacts.first().cloned();
                app.build_sha256 = Some(result.iso.sha256.clone());
                let label = result
                    .artifacts
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| result.output_dir.display().to_string());
                app.status =
                    format!("ISO ready: {label} — optional checks are available if you want them");
                // Pre-fill verify source with artifact.
                if let Some(art) = &app.build_artifact {
                    app.verify_source = art.display().to_string();
                }
            }
            WorkerMsg::VerifyOk(result) => {
                app.busy = false;
                app.verify_result = Some(*result);
                app.progress.verify_done = true;
                app.status = "Optional checksum check complete".into();
            }
            WorkerMsg::Iso9660Ok(result) => {
                app.busy = false;
                app.iso9660_result = Some(*result);
                app.progress.iso9660_done = true;
                app.status = "Optional ISO-9660 check complete".into();
            }
            WorkerMsg::OpError(e) => {
                app.busy = false;
                app.status = format!("Error: {e}");
                app.push_log(e, LogLevel::Error);
            }
        }
    }
}
