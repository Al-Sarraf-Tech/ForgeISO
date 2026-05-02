slint::include_modules!();

mod app;
mod config;
mod defaults;
mod handlers;
mod jobs;
mod obs;
mod persist;
mod profiles;
mod state;
mod worker;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use slint::ComponentHandle;

use app::{ForgeApp, APP};
use config::{make_preset_cards, make_profile_chips, preset_display_name};
use forgeiso_engine::ForgeIsoEngine;
use handlers::wire_all_handlers;
use persist::{load_state, save_state};
use state::{InjectState, PersistedState, VerifyState};

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    // JSON tracing — fail-open. Guard held for program lifetime.
    let _tracing_guard = obs::init_tracing();

    // OpenTelemetry tracing — feature-gated. Guard held for program lifetime
    // so the exporter flushes on Drop. With the `otel` feature off, this is a
    // zero-cost no-op guard.
    #[cfg(feature = "otel")]
    let _otel = forgeiso_engine::observability::init_otel(
        std::env::var("FORGEISO_OTEL_ENDPOINT").ok().as_deref(),
    );

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

    // Surface build metadata + restore persisted theme + status-bar toggles.
    {
        let theme = win.global::<Theme>();
        theme.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        theme.set_build_hash(option_env!("FORGEISO_BUILD_HASH").unwrap_or("").into());
        theme.set_license("MIT".into());
        // Restore persisted theme mode (defaults to "dark" if file missing/invalid).
        let mode = match saved.ui.theme.as_str() {
            "light" => "light",
            _ => "dark",
        };
        theme.set_mode(mode.into());
        // Restore status-bar visibility toggles (gear popup persists changes).
        theme.set_show_health_dot(saved.ui.show_health_dot);
        theme.set_show_version(saved.ui.show_version);
        theme.set_show_build_hash(saved.ui.show_build_hash);
        theme.set_show_license(saved.ui.show_license);
        theme.set_show_error_count(saved.ui.show_error_count);
        theme.set_compact_status_bar(saved.ui.compact_status_bar);
    }

    // Populate window from persisted state.
    restore_inject(&win, &saved.inject);
    restore_verify(&win, &saved.verify);
    let (presets_row1, presets_row2) = make_preset_cards();
    win.set_presets_row1(presets_row1);
    win.set_presets_row2(presets_row2);
    win.set_profile_chips(make_profile_chips());

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

    wire_all_handlers(&win);

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
        ui: APP
            .with(|cell| cell.borrow().as_ref().map(|rc| rc.borrow().snap_ui()))
            .unwrap_or_default(),
    };
    save_state(&state);

    Ok(())
}

// ── Restore helpers ───────────────────────────────────────────────────────────

pub(crate) fn restore_inject(w: &AppWindow, s: &InjectState) {
    let fs = w.global::<FormState>();
    fs.set_source_path(s.source.clone().into());
    fs.set_selected_preset(s.source_preset.clone().into());
    fs.set_selected_preset_name(
        preset_display_name(&s.source_preset)
            .unwrap_or_default()
            .into(),
    );
    // Restore configuration profile selection. Unknown ids round-trip via
    // ProfileKind::from_id → default_kind() to stay coherent with the UI.
    {
        let canonical = profiles::ProfileKind::from_id(&s.selected_profile)
            .unwrap_or_else(profiles::ProfileKind::default_kind);
        fs.set_selected_profile(canonical.as_id().into());
    }
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

pub(crate) fn restore_verify(w: &AppWindow, s: &VerifyState) {
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

pub(crate) fn copy_to_clipboard(text: &str) -> Result<(), &'static str> {
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

pub(crate) fn open_in_file_manager(path: &str) -> Result<(), &'static str> {
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
