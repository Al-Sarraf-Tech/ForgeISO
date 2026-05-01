//! Step 3 (Build) callback wiring.
//!
//! Covers: build-back, build-back-to-source, build-run, build-view-results.

use slint::ComponentHandle;

use crate::app::with_app;
use crate::{AppState, AppWindow};

pub(crate) fn wire(win: &AppWindow) {
    // build-back  — navigate to step 2
    {
        let weak = win.as_weak();
        win.on_build_back(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                if !gs.get_job_running() {
                    gs.set_current_step(2);
                }
            }
        });
    }

    // build-back-to-source  — navigate to step 1
    {
        let weak = win.as_weak();
        win.on_build_back_to_source(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                if !gs.get_job_running() {
                    gs.set_current_step(1);
                }
            }
        });
    }

    // build-run  — kick off the inject pipeline
    win.on_build_run(|| {
        with_app(|a| a.spawn_inject());
    });

    // build-view-results  — jump to check step
    {
        let weak = win.as_weak();
        win.on_build_view_results(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                if gs.get_step3_done() {
                    gs.set_current_step(4);
                }
            }
        });
    }
}
