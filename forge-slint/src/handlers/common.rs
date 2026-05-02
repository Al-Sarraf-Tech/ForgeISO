//! Cross-step callback wiring: cancel-job, doctor, theme, step-bar, log toggles.
//!
//! These callbacks are not specific to a single wizard step — they operate on
//! global UI chrome (status bar, log drawer, doctor panel) or the step bar.

use slint::ComponentHandle;

use crate::app::with_app;
use crate::{AppState, AppWindow, Theme};

pub(crate) fn wire(win: &AppWindow) {
    // cancel-job
    win.on_cancel_job(|| {
        with_app(|a| a.cancel_job());
    });

    // theme-toggle — flip dark/light mode and persist immediately
    {
        let weak = win.as_weak();
        win.on_theme_toggle(move || {
            if let Some(w) = weak.upgrade() {
                let theme = w.global::<Theme>();
                let next = if theme.get_mode() == "light" {
                    "dark"
                } else {
                    "light"
                };
                theme.set_mode(next.into());
                with_app(|a| a.persist_ui());
            }
        });
    }

    // persist-ui-settings — fired by the StatusBar gear popup whenever any
    // toggle flips. Snapshots the full Theme global state to disk so changes
    // survive restart.
    win.on_persist_ui_settings(|| {
        with_app(|a| a.persist_ui());
    });

    // doctor-toggle
    {
        let weak = win.as_weak();
        win.on_doctor_toggle(move || {
            if let Some(w) = weak.upgrade() {
                let g = w.global::<AppState>();
                if g.get_doctor_open() {
                    g.set_doctor_open(false);
                } else {
                    g.set_doctor_open(true);
                    with_app(|a| a.spawn_doctor());
                }
            }
        });
    }

    // step-bar-clicked  — free navigation when not building; locked during builds
    {
        let weak = win.as_weak();
        win.on_step_bar_clicked(move |step| {
            if let Some(w) = weak.upgrade() {
                let g = w.global::<AppState>();
                if g.get_job_running() {
                    return;
                }
                // Allow backward navigation freely. Forward navigation
                // requires that prerequisite steps are complete.
                let target = step;
                let allowed = match target {
                    1 => true,
                    2 => g.get_step1_done(),
                    3 => g.get_step2_done(),
                    4 => g.get_step3_done(),
                    _ => false,
                };
                if allowed {
                    g.set_current_step(target);
                }
            }
        });
    }

    // log-toggle
    {
        let weak = win.as_weak();
        win.on_log_toggle(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                let open = gs.get_log_open();
                gs.set_log_open(!open);
            }
        });
    }

    // log-filter-toggle
    {
        let weak = win.as_weak();
        win.on_log_filter_toggle(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                let errors_only = gs.get_log_errors_only();
                gs.set_log_errors_only(!errors_only);
            }
        });
    }
}
