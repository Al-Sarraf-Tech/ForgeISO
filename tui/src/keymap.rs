//! Keyboard input dispatch for the TUI wizard.
//!
//! Owns the per-step key handlers, plus a top-level [`handle_key`] that
//! consumes a `KeyEvent` and routes it through the various editing /
//! confirmation modes before delegating to the per-step handler.
//!
//! Returns [`KeyOutcome::Quit`] when the user has confirmed quitting and
//! [`KeyOutcome::Continue`] otherwise; `main()` translates `Quit` into a
//! loop break.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use forgeiso_engine::{all_presets, ForgeIsoEngine, IsoPreset};
use tokio::sync::mpsc;

use crate::state::{App, SourceFocus, WizardStep, WorkerMsg};

/// Result of processing a single key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyOutcome {
    /// Continue the run loop.
    Continue,
    /// User confirmed quit; the run loop should exit.
    Quit,
}

/// Top-level key dispatch. Mirrors the original inline match block in
/// `main()` exactly so behaviour is unchanged.
pub(crate) fn handle_key(
    app: &mut App,
    key: KeyEvent,
    engine: &Arc<ForgeIsoEngine>,
    tx: &mpsc::UnboundedSender<WorkerMsg>,
) -> KeyOutcome {
    if key.kind != KeyEventKind::Press {
        return KeyOutcome::Continue;
    }

    // Quit confirmation handling.
    if app.quit_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return KeyOutcome::Quit,
            _ => {
                app.quit_confirm = false;
                app.status = "Ready".into();
                return KeyOutcome::Continue;
            }
        }
    }

    // Ctrl-C always quits.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return KeyOutcome::Quit;
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
        return KeyOutcome::Continue;
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
        return KeyOutcome::Continue;
    }

    // Step-specific key handling.
    match app.step {
        WizardStep::Source => handle_source_keys(app, key.code),
        WizardStep::Configure => handle_configure_keys(app, key.code),
        WizardStep::Build => {
            handle_build_keys(app, key.code, engine, tx);
        }
        WizardStep::OptionalChecks => {
            handle_check_keys(app, key.code, engine, tx);
        }
    }

    KeyOutcome::Continue
}

pub(crate) fn handle_source_keys(app: &mut App, code: KeyCode) {
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

pub(crate) fn handle_configure_keys(app: &mut App, code: KeyCode) {
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

pub(crate) fn handle_build_keys(
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

pub(crate) fn handle_check_keys(
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
