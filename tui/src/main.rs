mod state;
mod ui;
mod worker;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use forgeiso_engine::{all_presets, ForgeIsoEngine, IsoPreset};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use state::{App, LogLevel, SourceFocus, WizardStep, WorkerMsg};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let mut app = App::new(engine.doctor().await);
    let (tx, mut rx_worker) = mpsc::unbounded_channel::<WorkerMsg>();

    loop {
        // Drain worker results.
        while let Ok(msg) = rx_worker.try_recv() {
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
                    app.status = format!(
                        "ISO ready: {label} — optional checks are available if you want them"
                    );
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

        terminal.draw(|f| ui::ui(f, &app))?;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Quit confirmation handling.
                if app.quit_confirm {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => break,
                        _ => {
                            app.quit_confirm = false;
                            app.status = "Ready".into();
                            continue;
                        }
                    }
                }

                // Ctrl-C always quits.
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                // Editing mode for text fields.
                if app.editing {
                    match key.code {
                        KeyCode::Esc => {
                            app.editing = false;
                        }
                        KeyCode::Enter => {
                            app.editing = false;
                        }
                        KeyCode::Backspace => {
                            let idx = app.field_index;
                            let mut val = app.get_field_string_raw(idx);
                            val.pop();
                            app.set_field_value(idx, val);
                        }
                        KeyCode::Char(ch) => {
                            let idx = app.field_index;
                            let mut val = app.get_field_string_raw(idx);
                            val.push(ch);
                            app.set_field_value(idx, val);
                        }
                        _ => {}
                    }
                    continue;
                }

                // Optional-checks step editing.
                if app.step == WizardStep::OptionalChecks && app.check_editing {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            app.check_editing = false;
                        }
                        KeyCode::Backspace => {
                            app.verify_source.pop();
                            app.invalidate_checks_only();
                        }
                        KeyCode::Char(ch) => {
                            app.verify_source.push(ch);
                            app.invalidate_checks_only();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Step-specific key handling.
                match app.step {
                    WizardStep::Source => handle_source_keys(&mut app, key.code),
                    WizardStep::Configure => handle_configure_keys(&mut app, key.code),
                    WizardStep::Build => {
                        handle_build_keys(&mut app, key.code, &engine, &tx);
                    }
                    WizardStep::OptionalChecks => {
                        handle_check_keys(&mut app, key.code, &engine, &tx);
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// ─── Key handlers ───────────────────────────────────────────────────────────

fn handle_source_keys(app: &mut App, code: KeyCode) {
    let presets = all_presets();
    match code {
        KeyCode::Char('q') => {
            app.quit_confirm = true;
            app.status = "Quit? Press y to confirm, any other key to cancel".into();
        }
        KeyCode::Tab => {
            app.source_focus = match app.source_focus {
                SourceFocus::PresetList => SourceFocus::ManualInput,
                SourceFocus::ManualInput => SourceFocus::PresetList,
            };
        }
        KeyCode::Up => {
            if app.source_focus == SourceFocus::PresetList && app.preset_scroll > 0 {
                app.preset_scroll -= 1;
                // Auto-select while scrolling.
                app.preset_selected = Some(app.preset_scroll);
                update_detected_distro(app, presets);
                app.progress.source_ready = !app.effective_source().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        KeyCode::Down => {
            if app.source_focus == SourceFocus::PresetList
                && app.preset_scroll < presets.len().saturating_sub(1)
            {
                app.preset_scroll += 1;
                app.preset_selected = Some(app.preset_scroll);
                update_detected_distro(app, presets);
                app.progress.source_ready = !app.effective_source().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        KeyCode::Enter => {
            if app.source_focus == SourceFocus::PresetList {
                app.preset_selected = Some(app.preset_scroll);
                update_detected_distro(app, presets);
                app.manual_source.clear();
                app.progress.source_ready = !app.effective_source().is_empty();
                app.invalidate_build_and_checks();
                app.status = format!(
                    "Selected: {}",
                    presets
                        .get(app.preset_scroll)
                        .map(|p| p.name)
                        .unwrap_or("?")
                );
            } else {
                // Toggle manual input editing — enter simple inline mode.
                app.source_focus = SourceFocus::ManualInput;
                // We handle manual input with direct char entry below.
            }
        }
        KeyCode::Right => {
            if app.effective_source().is_empty() {
                app.status = "Select a preset or enter a path/URL first".into();
            } else {
                app.progress.source_ready = true;
                app.step = WizardStep::Configure;
                app.status = "Source ready — continue with required settings".into();
            }
        }
        KeyCode::Char('n') if app.source_focus == SourceFocus::PresetList => {
            if app.effective_source().is_empty() {
                app.status = "Select a preset or enter a path/URL first".into();
            } else {
                app.progress.source_ready = true;
                app.step = WizardStep::Configure;
                app.status = "Source ready — continue with required settings".into();
            }
        }
        KeyCode::Char(ch) => {
            if app.source_focus == SourceFocus::ManualInput {
                app.manual_source.push(ch);
                app.preset_selected = None;
                app.progress.source_ready = !app.manual_source.trim().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        KeyCode::Backspace => {
            if app.source_focus == SourceFocus::ManualInput {
                app.manual_source.pop();
                app.progress.source_ready = !app.manual_source.trim().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        _ => {}
    }
}

fn update_detected_distro(app: &mut App, presets: &[IsoPreset]) {
    if let Some(idx) = app.preset_selected {
        if let Some(p) = presets.get(idx) {
            app.detected_distro = Some(p.distro.to_string());
            // Auto-set distro field for engine.
            app.distro = p.distro.to_string();
        }
    }
}

fn handle_configure_keys(app: &mut App, code: KeyCode) {
    let field_count = app.tab_field_count();
    match code {
        KeyCode::Char('q') => {
            app.quit_confirm = true;
            app.status = "Quit? Press y to confirm, any other key to cancel".into();
        }
        KeyCode::Tab => {
            app.config_tab = app.config_tab.next();
            app.field_index = 0;
        }
        KeyCode::Up => {
            if app.field_index > 0 {
                app.field_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.field_index + 1 < field_count {
                app.field_index += 1;
            }
        }
        KeyCode::Enter => {
            let fields = app.tab_fields();
            if let Some(f) = fields.get(app.field_index) {
                if f.is_toggle() {
                    app.toggle_field(app.field_index);
                } else {
                    app.editing = true;
                }
            }
        }
        KeyCode::Char(' ') => {
            let fields = app.tab_fields();
            if let Some(f) = fields.get(app.field_index) {
                if f.is_toggle() {
                    app.toggle_field(app.field_index);
                }
            }
        }
        KeyCode::Left | KeyCode::Char('b') => {
            app.step = WizardStep::Source;
            app.status = "Ready".into();
        }
        KeyCode::Right | KeyCode::Char('n') => {
            if let Some(err) = app.validate_step2() {
                app.status = format!("Validation: {err}");
            } else {
                app.progress.configure_done = true;
                app.step = WizardStep::Build;
                app.status = "Ready — review and press Enter to build".into();
            }
        }
        _ => {}
    }
}

fn handle_build_keys(
    app: &mut App,
    code: KeyCode,
    engine: &Arc<ForgeIsoEngine>,
    tx: &mpsc::UnboundedSender<WorkerMsg>,
) {
    match code {
        KeyCode::Char('q') => {
            if app.busy {
                app.quit_confirm = true;
                app.status =
                    "Build is running. Press y to force quit, any other key to cancel".into();
            } else {
                app.quit_confirm = true;
                app.status = "Quit? Press y to confirm, any other key to cancel".into();
            }
        }
        KeyCode::Enter if !app.busy && !app.build_is_complete() => {
            app.spawn_inject(Arc::clone(engine), tx.clone());
        }
        KeyCode::Char('r') if !app.busy && app.build_is_complete() => {
            app.spawn_inject(Arc::clone(engine), tx.clone());
        }
        KeyCode::Char('c') | KeyCode::Char('v') if app.build_is_complete() => {
            app.step = WizardStep::OptionalChecks;
            app.status = "Build complete — optional checks can add extra confidence".into();
        }
        KeyCode::Char('o') if app.build_is_complete() => {
            if let Some(art) = &app.build_artifact {
                if let Some(dir) = art.parent() {
                    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                }
            }
        }
        KeyCode::Left | KeyCode::Char('b') if !app.busy => {
            app.step = WizardStep::Configure;
            app.status = "Ready".into();
        }
        KeyCode::Right | KeyCode::Char('n') if app.build_is_complete() => {
            app.step = WizardStep::OptionalChecks;
            app.status = "Build complete — optional checks are available if you want them".into();
        }
        _ => {}
    }
}

fn handle_check_keys(
    app: &mut App,
    code: KeyCode,
    engine: &Arc<ForgeIsoEngine>,
    tx: &mpsc::UnboundedSender<WorkerMsg>,
) {
    match code {
        KeyCode::Char('q') => {
            app.quit_confirm = true;
            app.status = "Quit? Press y to confirm, any other key to cancel".into();
        }
        KeyCode::Up => {
            if app.check_field_index > 0 {
                app.check_field_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.check_field_index < 2 {
                app.check_field_index += 1;
            }
        }
        KeyCode::Enter => match app.check_field_index {
            0 => {
                app.check_editing = true;
            }
            1 if !app.busy => {
                app.spawn_verify(Arc::clone(engine), tx.clone());
            }
            2 if !app.busy => {
                app.spawn_iso9660(Arc::clone(engine), tx.clone());
            }
            _ => {}
        },
        KeyCode::Left | KeyCode::Char('b') if !app.busy => {
            app.step = WizardStep::Build;
            app.status = "Build complete — you can stop here or run optional checks later".into();
        }
        KeyCode::Backspace if app.check_field_index == 0 => {
            app.verify_source.pop();
            app.invalidate_checks_only();
        }
        KeyCode::Char(ch) if app.check_field_index == 0 => {
            app.verify_source.push(ch);
            app.invalidate_checks_only();
        }
        _ => {}
    }
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
