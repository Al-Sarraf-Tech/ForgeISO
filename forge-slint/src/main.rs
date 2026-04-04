slint::include_modules!();

mod app;
mod config;
mod defaults;
mod jobs;
mod persist;
mod state;
mod worker;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use slint::ComponentHandle;

use app::{with_app, with_app_result, ForgeApp, APP};
use config::{handle_preset_clicked, make_preset_cards, preset_display_name};
use forgeiso_engine::ForgeIsoEngine;
use persist::{load_state, save_state};
use state::{InjectState, PersistedState, VerifyState};

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    if !has_display_env(
        std::env::var_os("DISPLAY").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
    ) {
        anyhow::bail!(
            "No graphical display detected. Use `forgeiso-desktop` from a desktop session, or run `forgeiso-tui` / `forgeiso` on headless systems."
        );
    }

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
    let (presets_row1, presets_row2) = make_preset_cards();
    win.set_presets_row1(presets_row1);
    win.set_presets_row2(presets_row2);

    // Create app logic and register in thread-local.
    let app_rc = Rc::new(RefCell::new(ForgeApp::new(
        win.as_weak(),
        Arc::clone(&rt),
        Arc::clone(&engine),
    )));
    app_rc.borrow_mut().seed_default_edit_tracking(&win);
    APP.with(|cell| {
        *cell.borrow_mut() = Some(Rc::clone(&app_rc));
    });
    std::mem::drop(app_rc.borrow().subscribe_events());

    // Wire log model into the window.
    win.set_log_entries(app_rc.borrow().log_model.clone());

    // ── Callback wiring ───────────────────────────────────────────────────────

    // cancel-job
    win.on_cancel_job(|| {
        with_app(|a| a.cancel_job());
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
    let fs = w.global::<FormState>();
    fs.set_source_path(s.source.clone().into());
    fs.set_selected_preset(s.source_preset.clone().into());
    fs.set_selected_preset_name(
        preset_display_name(&s.source_preset)
            .unwrap_or_default()
            .into(),
    );
    fs.set_output_dir(s.output_dir.clone().into());
    fs.set_out_name(s.out_name.clone().into());
    fs.set_output_label(s.output_label.clone().into());
    fs.set_distro(s.distro.clone().into());
    fs.set_hostname(s.hostname.clone().into());
    fs.set_username(s.username.clone().into());
    // passwords intentionally NOT restored (#[serde(skip)])
    fs.set_password("".into());
    fs.set_password_confirm("".into());
    fs.set_realname(s.realname.clone().into());
    fs.set_ssh_keys(s.ssh_keys.clone().into());
    fs.set_ssh_password_auth(s.ssh_password_auth);
    fs.set_ssh_install_server(s.ssh_install_server);
    fs.set_dns_servers(s.dns_servers.clone().into());
    fs.set_ntp_servers(s.ntp_servers.clone().into());
    fs.set_static_ip(s.static_ip.clone().into());
    fs.set_gateway(s.gateway.clone().into());
    fs.set_http_proxy(s.http_proxy.clone().into());
    fs.set_https_proxy(s.https_proxy.clone().into());
    fs.set_no_proxy(s.no_proxy.clone().into());
    fs.set_timezone(s.timezone.clone().into());
    fs.set_locale(s.locale.clone().into());
    fs.set_keyboard_layout(s.keyboard_layout.clone().into());
    fs.set_storage_layout(s.storage_layout.clone().into());
    fs.set_apt_mirror(s.apt_mirror.clone().into());
    fs.set_packages(s.packages.clone().into());
    fs.set_apt_repos(s.apt_repos.clone().into());
    fs.set_dnf_repos(s.dnf_repos.clone().into());
    fs.set_run_commands(s.run_commands.clone().into());
    fs.set_late_commands(s.late_commands.clone().into());
    fs.set_firewall_enabled(s.firewall_enabled);
    fs.set_firewall_policy(s.firewall_policy.clone().into());
    fs.set_allow_ports(s.allow_ports.clone().into());
    fs.set_deny_ports(s.deny_ports.clone().into());
    fs.set_user_groups(s.user_groups.clone().into());
    fs.set_user_shell(s.user_shell.clone().into());
    fs.set_sudo_nopasswd(s.sudo_nopasswd);
    fs.set_enable_services(s.enable_services.clone().into());
    fs.set_disable_services(s.disable_services.clone().into());
    fs.set_docker(s.docker);
    fs.set_podman(s.podman);
    fs.set_docker_users(s.docker_users.clone().into());
    fs.set_swap_size_mb(s.swap_size_mb.clone().into());
    fs.set_encrypt(s.encrypt);
    // encrypt_passphrase intentionally NOT restored (#[serde(skip)])
    fs.set_encrypt_passphrase("".into());
    fs.set_mounts(s.mounts.clone().into());
    fs.set_grub_timeout(s.grub_timeout.clone().into());
    fs.set_grub_cmdline(s.grub_cmdline.clone().into());
    fs.set_grub_default(s.grub_default.clone().into());
    fs.set_sysctl_pairs(s.sysctl_pairs.clone().into());
    fs.set_dnf_mirror(s.dnf_mirror.clone().into());
    fs.set_pacman_repos(s.pacman_repos.clone().into());
    fs.set_pacman_mirror(s.pacman_mirror.clone().into());
    fs.set_sudo_commands(s.sudo_commands.clone().into());
    fs.set_swap_filename(s.swap_filename.clone().into());
    fs.set_swap_swappiness(s.swap_swappiness.clone().into());
    fs.set_wallpaper_path(s.wallpaper_path.clone().into());
    fs.set_no_user_interaction(s.no_user_interaction);
    fs.set_expected_sha256(s.expected_sha256.clone().into());
    let defaults_summary = if s.source_preset.is_empty() {
        String::new()
    } else {
        defaults::summary_for(&defaults::defaults_for(&s.distro, &s.source_preset))
    };
    let gs = w.global::<AppState>();
    gs.set_defaults_summary(defaults_summary.into());

    // Mark step 1 done if source path was restored.
    gs.set_step1_done(!s.source.is_empty());
    gs.set_passwords_match(true);
}

fn restore_verify(w: &AppWindow, s: &VerifyState) {
    let gs = w.global::<AppState>();
    gs.set_verify_source(s.source.clone().into());
    gs.set_sums_url(s.sums_url.clone().into());
}

pub(crate) fn clear_optional_checks(w: &AppWindow) {
    let gs = w.global::<AppState>();
    gs.set_verify_done(false);
    gs.set_verify_matched(false);
    gs.set_verify_hash_display("".into());
    gs.set_iso9660_done(false);
    gs.set_iso9660_compliant(false);
    gs.set_iso9660_boot_bios(false);
    gs.set_iso9660_boot_uefi(false);
    gs.set_iso9660_volume_id("".into());
}

pub(crate) fn clear_build_results(w: &AppWindow) {
    let gs = w.global::<AppState>();
    let artifact: String = gs.get_artifact_path().into();
    let verify_source: String = gs.get_verify_source().into();
    if !artifact.is_empty() && verify_source == artifact {
        gs.set_verify_source("".into());
    }
    gs.set_step3_done(false);
    gs.set_artifact_path("".into());
    gs.set_artifact_sha256("".into());
    clear_optional_checks(w);
}

// ── Clipboard helper ──────────────────────────────────────────────────────────

fn copy_to_clipboard(text: &str) -> Result<(), &'static str> {
    if let Some(message) = clipboard_unavailable_message(has_graphical_session()) {
        return Err(message);
    }
    for (program, args) in clipboard_programs(has_wayland_session()) {
        if try_write_command(program, args, text)? {
            return Ok(());
        }
    }

    Err("Clipboard helper not found — install wl-clipboard, xclip, or xsel")
}

fn open_in_file_manager(path: &str) -> Result<(), &'static str> {
    if !has_graphical_session() {
        return Err(
            "Open Folder requires a graphical session — open the output directory manually",
        );
    }

    for (program, args) in file_manager_programs() {
        let result = std::process::Command::new(program)
            .args(args)
            .arg(path)
            .output();

        match result {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(_) => {
                return Err("File manager launcher failed — open the output directory manually");
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("Failed to launch a file manager for the artifact directory"),
        }
    }

    Err("No file manager launcher found — install xdg-utils or gio")
}

fn has_graphical_session() -> bool {
    has_display_env(
        std::env::var_os("DISPLAY").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
    )
}

fn has_display_env(
    display: Option<&std::ffi::OsStr>,
    wayland_display: Option<&std::ffi::OsStr>,
) -> bool {
    display.is_some_and(|value| !value.is_empty())
        || wayland_display.is_some_and(|value| !value.is_empty())
}

fn has_wayland_session() -> bool {
    has_wayland_session_from(
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
    )
}

fn has_wayland_session_from(wayland_display: bool, session_type: Option<&str>) -> bool {
    wayland_display || session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
}

fn clipboard_unavailable_message(has_graphical_session: bool) -> Option<&'static str> {
    (!has_graphical_session).then_some(
        "Clipboard copy requires a graphical session — copy the SHA-256 manually from the field",
    )
}

fn try_write_command(program: &str, args: &[&str], text: &str) -> Result<bool, &'static str> {
    let spawned = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn();

    match spawned {
        Ok(mut child) => {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                if stdin.write_all(text.as_bytes()).is_err() {
                    return Err("Failed to write to the clipboard helper");
                }
            }
            match child.wait() {
                Ok(status) => Ok(status.success()),
                Err(_) => Err("Failed to wait for the clipboard helper"),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("Failed to launch the clipboard helper"),
    }
}

fn clipboard_programs(has_wayland: bool) -> Vec<(&'static str, &'static [&'static str])> {
    let mut programs = Vec::new();
    if has_wayland {
        programs.push(("wl-copy", &[][..]));
    }
    programs.push(("xclip", &["-selection", "clipboard"][..]));
    programs.push(("xsel", &["--clipboard", "--input"][..]));
    programs
}

fn file_manager_programs() -> [(&'static str, &'static [&'static str]); 2] {
    [("xdg-open", &[]), ("gio", &["open"])]
}

#[cfg(test)]
mod tests {
    use super::{
        clipboard_programs, clipboard_unavailable_message, file_manager_programs, has_display_env,
        has_wayland_session_from,
    };
    use std::ffi::OsStr;

    #[test]
    fn wayland_clipboard_prefers_wl_copy() {
        let programs = clipboard_programs(true);
        assert_eq!(programs[0].0, "wl-copy");
        assert_eq!(programs[1].0, "xclip");
        assert_eq!(programs[2].0, "xsel");
    }

    #[test]
    fn x11_clipboard_fallback_skips_wl_copy() {
        let programs = clipboard_programs(false);
        assert_eq!(programs[0].0, "xclip");
        assert_eq!(programs[1].0, "xsel");
    }

    #[test]
    fn wayland_detection_accepts_wayland_display_or_session_type() {
        assert!(has_wayland_session_from(true, None));
        assert!(has_wayland_session_from(false, Some("wayland")));
        assert!(!has_wayland_session_from(false, Some("x11")));
    }

    #[test]
    fn display_env_accepts_x11_or_wayland() {
        assert!(has_display_env(Some(OsStr::new(":0")), None));
        assert!(has_display_env(None, Some(OsStr::new("wayland-0"))));
    }

    #[test]
    fn display_env_rejects_missing_or_empty_values() {
        assert!(!has_display_env(None, None));
        assert!(!has_display_env(Some(OsStr::new("")), Some(OsStr::new(""))));
    }

    #[test]
    fn headless_clipboard_returns_helpful_error() {
        let err = clipboard_unavailable_message(false)
            .expect("headless copy should report a user-facing error");
        assert!(err.contains("graphical session"));
    }

    #[test]
    fn file_manager_prefers_xdg_open_then_gio() {
        let programs = file_manager_programs();
        assert_eq!(programs[0].0, "xdg-open");
        assert_eq!(programs[1].0, "gio");
    }
}
