//! Step 2 (Configure) callback wiring.
//!
//! Covers: browse-output-dir, configure-continue, configure-back,
//! apply-defaults, reset-defaults, field-edited, username-changed,
//! docker-changed.

use slint::ComponentHandle;

use crate::app::{with_app, with_app_result};
use crate::worker;
use crate::{AppState, AppWindow, FormState};

pub(crate) fn wire(win: &AppWindow) {
    // browse-output-dir
    {
        let weak = win.as_weak();
        win.on_browse_output_dir(move || {
            worker::pick_folder(weak.clone(), |w, path| {
                w.global::<FormState>().set_output_dir(path.into());
            });
        });
    }

    // configure-continue  — validate passwords + navigate to step 3
    {
        let weak = win.as_weak();
        win.on_configure_continue(move || {
            if let Some(w) = weak.upgrade() {
                let gs = w.global::<AppState>();
                if gs.get_job_running() {
                    return;
                }

                let fs = w.global::<FormState>();
                let pw: String = fs.get_password().into();
                let pc: String = fs.get_password_confirm().into();
                if fs.get_hostname().trim().is_empty() {
                    gs.set_status_text("Hostname is required".into());
                    gs.set_status_is_error(true);
                    return;
                }
                if fs.get_username().trim().is_empty() {
                    gs.set_status_text("Username is required".into());
                    gs.set_status_is_error(true);
                    return;
                }
                let match_ok = pw.is_empty() || pw == pc;
                gs.set_passwords_match(match_ok);
                if !match_ok {
                    gs.set_status_text("Passwords do not match".into());
                    gs.set_status_is_error(true);
                    return;
                }

                let validation = with_app_result(|a| a.validate_inject_form())
                    .unwrap_or_else(|| Err("application state is unavailable".to_string()));
                if let Err(msg) = validation {
                    gs.set_status_text(msg.into());
                    gs.set_status_is_error(true);
                    return;
                }

                gs.set_status_text("".into());
                gs.set_status_is_error(false);
                gs.set_step2_done(true);
                gs.set_current_step(3);
            }
        });
    }

    // configure-back  — navigate to step 1
    {
        let weak = win.as_weak();
        win.on_configure_back(move || {
            if let Some(w) = weak.upgrade() {
                w.global::<AppState>().set_current_step(1);
            }
        });
    }

    // apply-defaults  — apply distro defaults to unedited fields
    {
        let weak = win.as_weak();
        win.on_apply_defaults(move || {
            if let Some(w) = weak.upgrade() {
                with_app(|a| a.apply_distro_defaults(&w));
            }
        });
    }

    // reset-defaults  — clear edit tracking and reapply defaults
    {
        let weak = win.as_weak();
        win.on_reset_defaults(move || {
            if let Some(w) = weak.upgrade() {
                with_app(|a| a.reset_and_apply_defaults(&w));
            }
        });
    }

    // field-edited  — track which default-managed fields the user has touched
    win.on_field_edited(move |name| {
        let field: String = name.into();
        with_app(|a| a.mark_edited(&field));
    });

    // username-changed  — auto-manage groups and Docker user
    {
        let weak = win.as_weak();
        win.on_username_changed(move |_u| {
            if let Some(w) = weak.upgrade() {
                with_app(|a| a.on_username_changed(&w));
            }
        });
    }

    // docker-changed  — auto-manage Docker user membership when appropriate
    {
        let weak = win.as_weak();
        win.on_docker_changed(move || {
            if let Some(w) = weak.upgrade() {
                with_app(|a| a.on_docker_changed(&w));
            }
        });
    }
}
