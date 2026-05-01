mod keymap;
mod obs;
mod runtime;
mod state;
mod ui;
mod worker;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use forgeiso_engine::ForgeIsoEngine;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use keymap::KeyOutcome;
use state::{App, WorkerMsg};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // JSON tracing — fail-open. Guard held for program lifetime.
    let _tracing_guard = obs::init_tracing();

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let engine = Arc::new(ForgeIsoEngine::new());
    // Parallel structured-log channel — does not replace in-app log pane.
    let _trace_task = obs::spawn_event_tracer(&engine);
    let mut app = App::new(engine.doctor().await);
    let (tx, mut rx_worker) = mpsc::unbounded_channel::<WorkerMsg>();

    loop {
        runtime::drain_worker(&mut app, &mut rx_worker);

        terminal.draw(|f| ui::ui(f, &app))?;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                match keymap::handle_key(&mut app, key, &engine, &tx) {
                    KeyOutcome::Quit => break,
                    KeyOutcome::Continue => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::state::App;
    use crate::state::WizardStep;
    use crate::ui::help_text_for_step;
    use forgeiso_engine::{DoctorReport, GuidedWorkflowProgress, Iso9660Compliance, VerifyResult};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn sample_doctor_report() -> DoctorReport {
        DoctorReport {
            host_os: "linux".into(),
            host_arch: "x86_64".into(),
            linux_supported: true,
            tooling: BTreeMap::new(),
            warnings: Vec::new(),
            timestamp: "2026-03-10T00:00:00Z".into(),
            distro_readiness: BTreeMap::new(),
        }
    }

    #[test]
    fn invalidating_upstream_input_clears_build_and_optional_checks() {
        let mut app = App::new(sample_doctor_report());
        app.progress = GuidedWorkflowProgress {
            source_ready: true,
            configure_done: true,
            build_done: true,
            verify_done: true,
            iso9660_done: true,
        };
        app.build_artifact = Some(PathBuf::from("/tmp/forgeiso.iso"));
        app.build_sha256 = Some("deadbeef".into());
        app.verify_source = "/tmp/forgeiso.iso".into();
        app.verify_result = Some(VerifyResult {
            filename: "forgeiso.iso".into(),
            expected: "abc".into(),
            actual: "def".into(),
            matched: false,
        });
        app.iso9660_result = Some(Iso9660Compliance {
            compliant: true,
            volume_id: Some("FORGEISO".into()),
            size_bytes: 42,
            boot_bios: true,
            boot_uefi: true,
            el_torito_present: true,
            check_method: "iso9660_header".into(),
            error: None,
        });

        app.invalidate_build_and_checks();

        assert!(!app.progress.configure_done);
        assert!(!app.progress.build_done);
        assert!(!app.progress.verify_done);
        assert!(!app.progress.iso9660_done);
        assert!(app.build_artifact.is_none());
        assert!(app.build_sha256.is_none());
        assert!(app.verify_result.is_none());
        assert!(app.iso9660_result.is_none());
        assert!(app.verify_source.is_empty());
    }

    #[test]
    fn build_help_text_marks_optional_checks_as_post_build_work() {
        let help = help_text_for_step(WizardStep::Build, false, true);
        assert!(help.contains("optional checks"));
    }

    #[test]
    fn shared_step_labels_match_guided_product_model() {
        assert_eq!(WizardStep::Source.label(), "Choose ISO");
        assert_eq!(WizardStep::OptionalChecks.label(), "Optional Checks");
    }
}
