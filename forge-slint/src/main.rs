slint::include_modules!();

mod app;
mod persist;
mod state;
mod worker;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use slint::ComponentHandle;

use app::{handle_preset_clicked, make_preset_cards, with_app, ForgeApp, APP};
use forgeiso_engine::ForgeIsoEngine;
use persist::{load_state, save_state};
use state::{InjectState, PersistedState, VerifyState};

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Multi-threaded tokio runtime for engine async work.
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?,
    );

    // Engine — synchronous init.
    let engine = Arc::new(ForgeIsoEngine::new());

    // Load persisted form state (passwords excluded via #[serde(skip)]).
    let saved = load_state();

    // Create Slint window.
    let win = AppWindow::new()?;

    // Populate window from persisted state.
    restore_inject(&win, &saved.inject);
    restore_verify(&win, &saved.verify);
    win.set_presets(make_preset_cards());

    // Create app logic and register in thread-local.
    let app_rc = Rc::new(RefCell::new(ForgeApp::new(
        win.as_weak(),
        Arc::clone(&rt),
        Arc::clone(&engine),
    )));
    APP.with(|cell| {
        *cell.borrow_mut() = Some(Rc::clone(&app_rc));
    });

    // Wire log model into the window.
    win.set_log_entries(app_rc.borrow().log_model.clone());

    // ── Callback wiring ───────────────────────────────────────────────────────

    // cancel-job
    win.on_cancel_job(|| with_app(|a| a.cancel_job()));

    // doctor-toggle
    {
        let weak = win.as_weak();
        win.on_doctor_toggle(move || {
            if let Some(w) = weak.upgrade() {
                if w.get_doctor_open() {
                    w.set_doctor_open(false);
                } else {
                    w.set_doctor_open(true);
                    with_app(|a| a.spawn_doctor());
                }
            }
        });
    }

    // step-bar-clicked  — gated: forward only to completed/current steps
    {
        let weak = win.as_weak();
        win.on_step_bar_clicked(move |step| {
            if let Some(w) = weak.upgrade() {
                if w.get_job_running() {
                    return;
                }
                let current = w.get_current_step();
                let can = match step {
                    1 => true,
                    2 => w.get_step1_done() || current >= 2,
                    3 => w.get_step2_done() || current >= 3,
                    4 => w.get_step3_done() || current >= 4,
                    _ => false,
                };
                if can {
                    w.set_current_step(step);
                }
            }
        });
    }

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
                    w.set_source_path(path.clone().into());
                    w.set_step1_done(true);
                    // Access ForgeApp via thread-local — no Rc captured.
                    with_app(|a| a.spawn_detect_iso(path));
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
                w.set_step1_done(not_empty);
            }
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
                if !w.get_source_path().is_empty() {
                    w.set_step1_done(true);
                    w.set_current_step(2);
                }
            }
        });
    }

    // clear-source  — reset step 1 + abort any running tasks
    {
        let weak = win.as_weak();
        win.on_clear_source(move || {
            if let Some(w) = weak.upgrade() {
                w.set_source_path("".into());
                w.set_selected_preset("".into());
                w.set_detected_distro("".into());
                w.set_step1_done(false);
                w.set_step2_done(false);
                w.set_artifact_path("".into());
                w.set_artifact_sha256("".into());
                w.set_verify_done(false);
                w.set_iso9660_done(false);
                w.set_current_step(1);
                w.set_status_text("".into());
                w.set_status_is_error(false);
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
                a.finish_job();
            });
        });
    }

    // browse-output-dir
    {
        let weak = win.as_weak();
        win.on_browse_output_dir(move || {
            worker::pick_folder(weak.clone(), |w, path| {
                w.set_output_dir(path.into());
            });
        });
    }

    // configure-continue  — validate passwords + navigate to step 3
    {
        let weak = win.as_weak();
        win.on_configure_continue(move || {
            if let Some(w) = weak.upgrade() {
                if w.get_job_running() {
                    return;
                }

                let pw: String = w.get_password().into();
                let pc: String = w.get_password_confirm().into();
                let match_ok = pw.is_empty() || pw == pc;
                w.set_passwords_match(match_ok);
                if !match_ok {
                    w.set_status_text("Passwords do not match".into());
                    w.set_status_is_error(true);
                    return;
                }

                w.set_status_text("".into());
                w.set_status_is_error(false);
                w.set_current_step(3);
            }
        });
    }

    // configure-back  — navigate to step 1
    {
        let weak = win.as_weak();
        win.on_configure_back(move || {
            if let Some(w) = weak.upgrade() {
                w.set_current_step(1);
            }
        });
    }

    // build-back  — navigate to step 2
    {
        let weak = win.as_weak();
        win.on_build_back(move || {
            if let Some(w) = weak.upgrade() {
                if !w.get_job_running() {
                    w.set_current_step(2);
                }
            }
        });
    }

    // build-run  — kick off the inject pipeline
    win.on_build_run(|| with_app(|a| a.spawn_inject()));

    // build-view-results  — jump to check step
    {
        let weak = win.as_weak();
        win.on_build_view_results(move || {
            if let Some(w) = weak.upgrade() {
                w.set_current_step(4);
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
                w.set_verify_source(path.into());
                w.set_verify_done(false);
                w.set_iso9660_done(false);
            });
        });
    }

    // run-verify
    win.on_run_verify(|| with_app(|a| a.spawn_verify()));

    // run-iso9660
    win.on_run_iso9660(|| with_app(|a| a.spawn_iso9660()));

    // copy-sha256  — write artifact hash to clipboard via xclip/xsel
    {
        let weak = win.as_weak();
        win.on_copy_sha256(move || {
            if let Some(w) = weak.upgrade() {
                let hash: String = w.get_artifact_sha256().into();
                if !hash.is_empty() {
                    copy_to_clipboard(&hash);
                    w.set_status_text("SHA-256 copied to clipboard".into());
                    w.set_status_is_error(false);
                }
            }
        });
    }

    // open-folder  — reveal artifact directory in file manager
    {
        let weak = win.as_weak();
        win.on_open_folder(move || {
            if let Some(w) = weak.upgrade() {
                let path: String = w.get_artifact_path().into();
                if !path.is_empty() {
                    let dir = std::path::Path::new(&path)
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(path);
                    std::process::Command::new("xdg-open")
                        .arg(&dir)
                        .spawn()
                        .ok();
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
                a.finish_job();
            });
            if let Some(w) = weak.upgrade() {
                restore_inject(&w, &InjectState::default());
                restore_verify(&w, &VerifyState::default());
                w.set_step1_done(false);
                w.set_step2_done(false);
                w.set_step3_done(false);
                w.set_artifact_path("".into());
                w.set_artifact_sha256("".into());
                w.set_verify_done(false);
                w.set_iso9660_done(false);
                w.set_current_step(1);
                w.set_status_text("".into());
                w.set_status_is_error(false);
                w.set_passwords_match(true);
            }
        });
    }

    // log-toggle
    {
        let weak = win.as_weak();
        win.on_log_toggle(move || {
            if let Some(w) = weak.upgrade() {
                let open = w.get_log_open();
                w.set_log_open(!open);
            }
        });
    }

    // log-filter-toggle
    {
        let weak = win.as_weak();
        win.on_log_filter_toggle(move || {
            if let Some(w) = weak.upgrade() {
                let errors_only = w.get_log_errors_only();
                w.set_log_errors_only(!errors_only);
            }
        });
    }

    // ── Run event loop ────────────────────────────────────────────────────────

    win.run()?;

    // ── Persist form state on close (passwords excluded by #[serde(skip)]) ───

    let state = PersistedState {
        inject: APP
            .with(|cell| {
                cell.borrow()
                    .as_ref()
                    .and_then(|rc| rc.borrow().snap_inject())
            })
            .unwrap_or_default(),
        verify: APP
            .with(|cell| {
                cell.borrow()
                    .as_ref()
                    .and_then(|rc| rc.borrow().snap_verify())
            })
            .unwrap_or_default(),
    };
    save_state(&state);

    Ok(())
}

// ── Restore helpers ───────────────────────────────────────────────────────────

fn restore_inject(w: &AppWindow, s: &InjectState) {
    w.set_source_path(s.source.clone().into());
    w.set_selected_preset(s.source_preset.clone().into());
    w.set_output_dir(s.output_dir.clone().into());
    w.set_out_name(s.out_name.clone().into());
    w.set_output_label(s.output_label.clone().into());
    w.set_distro(s.distro.clone().into());
    w.set_hostname(s.hostname.clone().into());
    w.set_username(s.username.clone().into());
    // passwords intentionally NOT restored (#[serde(skip)])
    w.set_realname(s.realname.clone().into());
    w.set_ssh_keys(s.ssh_keys.clone().into());
    w.set_ssh_password_auth(s.ssh_password_auth);
    w.set_ssh_install_server(s.ssh_install_server);
    w.set_dns_servers(s.dns_servers.clone().into());
    w.set_ntp_servers(s.ntp_servers.clone().into());
    w.set_static_ip(s.static_ip.clone().into());
    w.set_gateway(s.gateway.clone().into());
    w.set_http_proxy(s.http_proxy.clone().into());
    w.set_https_proxy(s.https_proxy.clone().into());
    w.set_no_proxy(s.no_proxy.clone().into());
    w.set_timezone(s.timezone.clone().into());
    w.set_locale(s.locale.clone().into());
    w.set_keyboard_layout(s.keyboard_layout.clone().into());
    w.set_storage_layout(s.storage_layout.clone().into());
    w.set_apt_mirror(s.apt_mirror.clone().into());
    w.set_packages(s.packages.clone().into());
    w.set_apt_repos(s.apt_repos.clone().into());
    w.set_dnf_repos(s.dnf_repos.clone().into());
    w.set_run_commands(s.run_commands.clone().into());
    w.set_late_commands(s.late_commands.clone().into());
    w.set_firewall_enabled(s.firewall_enabled);
    w.set_firewall_policy(s.firewall_policy.clone().into());
    w.set_allow_ports(s.allow_ports.clone().into());
    w.set_deny_ports(s.deny_ports.clone().into());
    w.set_user_groups(s.user_groups.clone().into());
    w.set_user_shell(s.user_shell.clone().into());
    w.set_sudo_nopasswd(s.sudo_nopasswd);
    w.set_enable_services(s.enable_services.clone().into());
    w.set_disable_services(s.disable_services.clone().into());
    w.set_docker(s.docker);
    w.set_podman(s.podman);
    w.set_docker_users(s.docker_users.clone().into());
    w.set_swap_size_mb(s.swap_size_mb.clone().into());
    w.set_encrypt(s.encrypt);
    // encrypt_passphrase intentionally NOT restored (#[serde(skip)])
    w.set_mounts(s.mounts.clone().into());
    w.set_grub_timeout(s.grub_timeout.clone().into());
    w.set_grub_cmdline(s.grub_cmdline.clone().into());
    w.set_sysctl_pairs(s.sysctl_pairs.clone().into());
    w.set_no_user_interaction(s.no_user_interaction);
    w.set_expected_sha256(s.expected_sha256.clone().into());

    // Mark step 1 done if source path was restored.
    w.set_step1_done(!s.source.is_empty());
    w.set_passwords_match(true);
}

fn restore_verify(w: &AppWindow, s: &VerifyState) {
    w.set_verify_source(s.source.clone().into());
    w.set_sums_url(s.sums_url.clone().into());
}

// ── Clipboard helper ──────────────────────────────────────────────────────────

fn copy_to_clipboard(text: &str) {
    // Try xclip first, fall back to xsel.
    let ok = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.as_mut().map(|s| s.write_all(text.as_bytes()));
            c.wait()
        })
        .map(|s| s.success())
        .unwrap_or(false);

    if !ok {
        std::process::Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.as_mut().map(|s| s.write_all(text.as_bytes()));
                c.wait()
            })
            .ok();
    }
}
