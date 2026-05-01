//! Step 1 (Source) callback wiring.
//!
//! Covers: preset-clicked, browse-source, source-changed, source-continue,
//! clear-source.

use slint::ComponentHandle;

use crate::app::with_app;
use crate::clear_build_results;
use crate::config::handle_preset_clicked;
use crate::worker;
use crate::{AppState, AppWindow, FormState};

pub(crate) fn wire(win: &AppWindow) {
    // preset-clicked
    {
        let weak = win.as_weak();
        win.on_preset_clicked(move |id| {
            if let Some(w) = weak.upgrade() {
                with_app(|a| handle_preset_clicked(&w, id.as_str(), a));
            }
        });
    }

    // browse-source  — spawn zenity; on_picked runs via invoke_from_event_loop
    {
        let weak = win.as_weak();
        win.on_browse_source(move || {
            worker::pick_iso(
                weak.clone(),
                // This closure is Send + 'static. It is called on the event loop
                // thread (inside invoke_from_event_loop in handle_zenity).
                |w, path| {
                    let fs = w.global::<FormState>();
                    fs.set_source_path(path.clone().into());
                    fs.set_selected_preset("".into());
                    fs.set_selected_preset_name("".into());
                    fs.set_detected_distro("".into());
                    let gs = w.global::<AppState>();
                    gs.set_defaults_summary("".into());
                    gs.set_step1_done(true);
                    gs.set_step2_done(false);
                    clear_build_results(&w);
                    // Access ForgeApp via thread-local — no Rc captured.
                    with_app(|a| {
                        a.clear_defaults_state();
                        a.spawn_detect_iso(path);
                    });
                },
            );
        });
    }

    // source-changed  — typed path; trigger detect + mark done
    {
        let weak = win.as_weak();
        win.on_source_changed(move |text| {
            let t: String = text.into();
            let not_empty = !t.trim().is_empty();
            if let Some(w) = weak.upgrade() {
                let fs = w.global::<FormState>();
                fs.set_selected_preset("".into());
                fs.set_selected_preset_name("".into());
                fs.set_detected_distro("".into());
                let gs = w.global::<AppState>();
                gs.set_defaults_summary("".into());
                gs.set_step1_done(not_empty);
                gs.set_step2_done(false);
                clear_build_results(&w);
            }
            with_app(|a| a.clear_defaults_state());
            if not_empty {
                with_app(|a| a.spawn_detect_iso(t));
            }
        });
    }

    // source-continue  — navigate to step 2
    {
        let weak = win.as_weak();
        win.on_source_continue(move || {
            if let Some(w) = weak.upgrade() {
                if !w.global::<FormState>().get_source_path().is_empty() {
                    let gs = w.global::<AppState>();
                    gs.set_step1_done(true);
                    gs.set_current_step(2);
                }
            }
        });
    }

    // clear-source  — reset step 1 + abort any running tasks
    {
        let weak = win.as_weak();
        win.on_clear_source(move || {
            if let Some(w) = weak.upgrade() {
                let fs = w.global::<FormState>();
                fs.set_source_path("".into());
                fs.set_selected_preset("".into());
                fs.set_selected_preset_name("".into());
                fs.set_detected_distro("".into());
                let gs = w.global::<AppState>();
                gs.set_defaults_summary("".into());
                gs.set_step1_done(false);
                gs.set_step2_done(false);
                clear_build_results(&w);
                gs.set_current_step(1);
                gs.set_status_text("".into());
                gs.set_status_is_error(false);
                gs.set_passwords_match(true);
            }
            with_app(|a| {
                if let Some(h) = a.detect_task.take() {
                    h.abort();
                }
                if let Some(h) = a.current_task.take() {
                    h.abort();
                }
                if let Some(h) = a.sha256_task.take() {
                    h.abort();
                }
                a.clear_defaults_state();
                a.finish_job();
            });
        });
    }
}
