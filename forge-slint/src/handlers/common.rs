//! Cross-step callback wiring: cancel-job, doctor, theme, step-bar, log toggles.
//!
//! These callbacks are not specific to a single wizard step — they operate on
//! global UI chrome (status bar, log drawer, doctor panel) or the step bar.

use slint::ComponentHandle;

use crate::app::with_app;
use crate::config::{make_compare_rows, profile_kind_for};
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

    // compare-open — open the compare-profiles modal. AppState slots already
    // hold the A/B pair from prior interaction (or the startup defaults), so
    // this just flips the modal visible.
    {
        let weak = win.as_weak();
        win.on_compare_open(move || {
            if let Some(w) = weak.upgrade() {
                w.global::<AppState>().set_compare_modal_open(true);
            }
        });
    }

    // compare-select-a / compare-select-b — change one side of the compared
    // pair, recompute the diff rows, and push back. The AppState id slot is
    // updated first so the chip's "selected" highlight reacts before the row
    // model has rebuilt.
    {
        let weak = win.as_weak();
        win.on_compare_select_a(move |id| {
            if let Some(w) = weak.upgrade() {
                let id_str: String = id.into();
                w.global::<AppState>()
                    .set_compare_profile_a(id_str.clone().into());
                let a = profile_kind_for(&id_str);
                let b_str: String = w.global::<AppState>().get_compare_profile_b().into();
                let b = profile_kind_for(&b_str);
                w.set_compare_rows(make_compare_rows(a, b));
            }
        });
    }
    {
        let weak = win.as_weak();
        win.on_compare_select_b(move |id| {
            if let Some(w) = weak.upgrade() {
                let id_str: String = id.into();
                w.global::<AppState>()
                    .set_compare_profile_b(id_str.clone().into());
                let a_str: String = w.global::<AppState>().get_compare_profile_a().into();
                let a = profile_kind_for(&a_str);
                let b = profile_kind_for(&id_str);
                w.set_compare_rows(make_compare_rows(a, b));
            }
        });
    }

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
