//! Step 4 (Check) callback wiring.
//!
//! Covers: browse-verify-source, check-back, run-verify, run-iso9660,
//! run-verify-output, copy-sha256, open-folder, clear-forms.

use slint::ComponentHandle;

use crate::app::with_app;
use crate::state::{InjectState, VerifyState};
use crate::{
    clear_build_results, clear_optional_checks, copy_to_clipboard, jobs, open_in_file_manager,
    restore_inject, restore_verify, worker, AppState, AppWindow,
};

pub(crate) fn wire(win: &AppWindow) {
    // check-back  — return to build summary
    {
        let weak = win.as_weak();
        win.on_check_back(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                if !gs.get_job_running() && gs.get_step3_done() {
                    gs.set_current_step(3);
                }
            }
        });
    }

    // browse-verify-source
    {
        let weak = win.as_weak();
        win.on_browse_verify_source(move || {
            // Abort current verify/iso9660 task when source changes.
            with_app(|a| {
                if let Some(h) = a.current_task.take() {
                    h.abort();
                }
                a.finish_job();
            });
            worker::pick_iso(weak.clone(), |w, path| {
                w.global::<AppState>().set_verify_source(path.into());
                clear_optional_checks(&w);
            });
        });
    }

    // run-verify
    win.on_run_verify(|| {
        with_app(|a| a.spawn_verify());
    });

    // run-iso9660
    win.on_run_iso9660(|| {
        with_app(|a| a.spawn_iso9660());
    });

    // run-verify-output — re-hash output ISO to confirm write integrity
    {
        let weak = win.as_weak();
        win.on_run_verify_output(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                let path = gs.get_artifact_path().to_string();
                let hash = gs.get_artifact_sha256().to_string();
                if !path.is_empty() && !hash.is_empty() {
                    jobs::spawn_verify_output(w.as_weak(), path, hash);
                }
            }
        });
    }

    // copy-sha256  — write artifact hash to clipboard via wl-copy/xclip/xsel
    {
        let weak = win.as_weak();
        win.on_copy_sha256(move || {
            if let Some(w) = weak.upgrade() {
                let hash: String = w.global::<AppState>().get_artifact_sha256().into();
                if !hash.is_empty() {
                    let gs = w.global::<AppState>();
                    match copy_to_clipboard(&hash) {
                        Ok(()) => {
                            gs.set_status_text("SHA-256 copied to clipboard".into());
                            gs.set_status_is_error(false);
                        }
                        Err(msg) => {
                            gs.set_status_text(msg.into());
                            gs.set_status_is_error(true);
                        }
                    }
                }
            }
        });
    }

    // open-folder  — reveal artifact directory in file manager
    {
        let weak = win.as_weak();
        win.on_open_folder(move || {
            if let Some(w) = weak.upgrade() {
                let path: String = w.global::<AppState>().get_artifact_path().into();
                if !path.is_empty() {
                    let dir = std::path::Path::new(&path)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(path);
                    let gs = w.global::<AppState>();
                    match open_in_file_manager(&dir) {
                        Ok(()) => {
                            gs.set_status_text("Opened artifact folder".into());
                            gs.set_status_is_error(false);
                        }
                        Err(msg) => {
                            gs.set_status_text(msg.into());
                            gs.set_status_is_error(true);
                        }
                    }
                }
            }
        });
    }

    // clear-forms  — reset everything back to defaults
    {
        let weak = win.as_weak();
        win.on_clear_forms(move || {
            with_app(|a| {
                if let Some(h) = a.current_task.take() {
                    h.abort();
                }
                if let Some(h) = a.detect_task.take() {
                    h.abort();
                }
                if let Some(h) = a.sha256_task.take() {
                    h.abort();
                }
                a.edited_fields.clear();
                a.finish_job();
            });
            if let Some(w) = weak.upgrade() {
                restore_inject(&w, &InjectState::default());
                restore_verify(&w, &VerifyState::default());
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
        });
    }
}
