use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use forgeiso_engine::{
    find_preset_by_str, resolve_url, AcquisitionStrategy, ContainerConfig, Distro, EventLevel,
    FirewallConfig, ForgeIsoEngine, GrubConfig, InjectConfig, IsoSource, NetworkConfig,
    ProxyConfig, SshConfig, SwapConfig, UserConfig,
};
use sha2::{Digest, Sha256};
use slint::{ComponentHandle, Model, ModelRc, VecModel, Weak};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use std::collections::HashSet;

use crate::defaults;
use crate::state::{lines, opt, tokens, InjectState, VerifyState};
use crate::{clear_build_results, AppWindow, LogEntry, PresetCard};

// ── Thread-local app handle ───────────────────────────────────────────────────
//
// Lives here (not main.rs) so spawn_* event-listener closures can call
// `with_app()` from inside `invoke_from_event_loop` without capturing an Rc.

thread_local! {
    pub static APP: RefCell<Option<Rc<RefCell<ForgeApp>>>> = const { RefCell::new(None) };
}

/// Run `f` on the event-loop thread with a mutable reference to ForgeApp.
pub fn with_app<F: FnOnce(&mut ForgeApp)>(f: F) {
    APP.with(|cell| {
        if let Some(rc) = cell.borrow().as_ref() {
            f(&mut rc.borrow_mut());
        }
    });
}

/// Run `f` on the event-loop thread with a mutable reference to ForgeApp and return a value.
pub fn with_app_result<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut ForgeApp) -> R,
{
    APP.with(|cell| cell.borrow().as_ref().map(|rc| f(&mut rc.borrow_mut())))
}

// ── Preset cards shown on Step 1 ─────────────────────────────────────────────

pub fn make_preset_cards() -> ModelRc<PresetCard> {
    let cards: Vec<PresetCard> = vec![
        PresetCard {
            id: "ubuntu-server-lts".into(),
            emoji: "🐧".into(),
            name: "Ubuntu Server".into(),
            desc: "LTS Server".into(),
        },
        PresetCard {
            id: "ubuntu-desktop-lts".into(),
            emoji: "🖥️".into(),
            name: "Ubuntu Desktop".into(),
            desc: "LTS Desktop".into(),
        },
        PresetCard {
            id: "linux-mint-cinnamon".into(),
            emoji: "🌿".into(),
            name: "Linux Mint".into(),
            desc: "Cinnamon".into(),
        },
        PresetCard {
            id: "fedora-server".into(),
            emoji: "🎩".into(),
            name: "Fedora Server".into(),
            desc: "Latest stable".into(),
        },
        PresetCard {
            id: "rocky-linux".into(),
            emoji: "🪨".into(),
            name: "Rocky Linux".into(),
            desc: "RHEL compatible".into(),
        },
        PresetCard {
            id: "almalinux".into(),
            emoji: "🦬".into(),
            name: "AlmaLinux".into(),
            desc: "RHEL compatible".into(),
        },
        PresetCard {
            id: "arch-linux".into(),
            emoji: "⚙️".into(),
            name: "Arch Linux".into(),
            desc: "Rolling release".into(),
        },
        PresetCard {
            id: "fedora-workstation".into(),
            emoji: "💻".into(),
            name: "Fedora WS".into(),
            desc: "Workstation".into(),
        },
    ];
    ModelRc::new(VecModel::from(cards))
}

pub fn preset_display_name(id: &str) -> Option<&'static str> {
    find_preset_by_str(id).map(|preset| preset.name)
}

// ── Log helpers ───────────────────────────────────────────────────────────────

const MAX_LOG: usize = 5_000;

fn now_ts() -> slint::SharedString {
    chrono::Local::now().format("%H:%M:%S").to_string().into()
}

// ── Main app state ────────────────────────────────────────────────────────────

pub struct ForgeApp {
    pub win: Weak<AppWindow>,
    pub rt: Arc<Runtime>,
    pub engine: Arc<ForgeIsoEngine>,
    // Task handles for cancellation
    pub current_task: Option<JoinHandle<()>>,
    pub detect_task: Option<JoinHandle<()>>,
    pub sha256_task: Option<JoinHandle<()>>,
    // Log storage — Rc<VecModel> kept for direct mutation; ModelRc wraps same allocation.
    pub log_vec: Rc<VecModel<LogEntry>>,
    pub log_model: ModelRc<LogEntry>,
    // Download progress tracking (phase → log index for in-place update)
    pub download_idx: std::collections::HashMap<String, usize>,
    // Tracks which default-managed fields the user has manually edited.
    pub edited_fields: HashSet<String>,
}

impl ForgeApp {
    pub fn new(win: Weak<AppWindow>, rt: Arc<Runtime>, engine: Arc<ForgeIsoEngine>) -> Self {
        let log_vec = Rc::new(VecModel::from(Vec::<LogEntry>::new()));
        let log_model = ModelRc::from(Rc::clone(&log_vec));
        Self {
            win,
            rt,
            engine,
            current_task: None,
            detect_task: None,
            sha256_task: None,
            log_vec,
            log_model,
            download_idx: std::collections::HashMap::new(),
            edited_fields: HashSet::new(),
        }
    }

    // ── Job lifecycle ─────────────────────────────────────────────────────────

    fn start_job(&mut self, phase: &str) {
        self.download_idx.clear();
        if let Some(w) = self.win.upgrade() {
            w.set_job_running(true);
            w.set_job_phase(phase.into());
            w.set_job_percent(-1.0);
            w.set_status_text("".into());
        }
    }

    pub fn finish_job(&self) {
        if let Some(w) = self.win.upgrade() {
            w.set_job_running(false);
            w.set_job_phase("".into());
            w.set_job_percent(-1.0);
        }
    }

    pub fn set_status_ok(&self, msg: impl Into<String>) {
        if let Some(w) = self.win.upgrade() {
            w.set_status_text(msg.into().into());
            w.set_status_is_error(false);
        }
    }

    pub fn set_status_err(&self, msg: impl Into<String>) {
        let s: String = msg.into();
        let first = s
            .lines()
            .next()
            .unwrap_or(&s)
            .chars()
            .take(200)
            .collect::<String>();
        if let Some(w) = self.win.upgrade() {
            w.set_status_text(first.into());
            w.set_status_is_error(true);
        }
    }

    /// Spawn a task that receives engine broadcast events and delivers them to
    /// the log panel via `invoke_from_event_loop`. Returns a handle that callers
    /// should abort once the main operation task finishes.
    pub fn subscribe_events(&self) -> JoinHandle<()> {
        let mut rx = self.engine.subscribe();
        self.rt.spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let phase = format!("{:?}", ev.phase);
                let msg = ev.message.clone();
                let level = match ev.level {
                    EventLevel::Error => 2i32,
                    EventLevel::Warn => 1i32,
                    _ => 0i32,
                };
                let pct = ev.percent;
                let _ = slint::invoke_from_event_loop(move || {
                    APP.with(|cell| {
                        if let Some(rc) = cell.borrow().as_ref() {
                            rc.borrow_mut().push_log(&phase, &msg, level, pct);
                        }
                    });
                });
            }
        })
    }

    pub fn push_log(&mut self, phase: &str, message: &str, level: i32, percent: Option<f32>) {
        let m = &self.log_vec;

        // For download progress: update in-place rather than append.
        if let Some(pct) = percent {
            if pct >= 0.0 {
                if let Some(&idx) = self.download_idx.get(phase) {
                    if idx < m.row_count() {
                        let mut entry = m.row_data(idx).unwrap();
                        entry.message = format!("{message} ({:.0}%)", pct * 100.0).into();
                        m.set_row_data(idx, entry);
                        return;
                    }
                }
            }
        }

        let idx = m.row_count();
        m.push(LogEntry {
            phase: phase.into(),
            message: message.into(),
            level,
            timestamp: now_ts(),
        });

        // Track download phase index for in-place progress updates.
        if percent.is_some() {
            self.download_idx.insert(phase.to_string(), idx);
        }

        // Evict oldest 20% when over limit.
        if m.row_count() > MAX_LOG {
            let evict = MAX_LOG / 5;
            for _ in 0..evict {
                m.remove(0);
            }
            self.download_idx.clear();
        }

        // Update error badge count.
        if level == 2 {
            if let Some(w) = self.win.upgrade() {
                w.set_log_error_count(w.get_log_error_count() + 1);
            }
        }
    }

    pub fn cancel_job(&mut self) {
        if let Some(h) = self.current_task.take() {
            h.abort();
        }
        if let Some(h) = self.detect_task.take() {
            h.abort();
        }
        if let Some(h) = self.sha256_task.take() {
            h.abort();
        }
        self.finish_job();
        self.set_status_ok("Cancelled");
    }

    // ── Snapshot current form state from Slint ────────────────────────────────

    pub fn snap_inject(&self) -> Option<InjectState> {
        let w = self.win.upgrade()?;
        Some(InjectState {
            source: w.get_source_path().into(),
            source_preset: w.get_selected_preset().into(),
            output_dir: w.get_output_dir().into(),
            out_name: w.get_out_name().into(),
            output_label: w.get_output_label().into(),
            distro: w.get_distro().into(),
            hostname: w.get_hostname().into(),
            username: w.get_username().into(),
            password: w.get_password().into(),
            password_confirm: w.get_password_confirm().into(),
            realname: w.get_realname().into(),
            ssh_keys: w.get_ssh_keys().into(),
            ssh_password_auth: w.get_ssh_password_auth(),
            ssh_install_server: w.get_ssh_install_server(),
            dns_servers: w.get_dns_servers().into(),
            ntp_servers: w.get_ntp_servers().into(),
            static_ip: w.get_static_ip().into(),
            gateway: w.get_gateway().into(),
            http_proxy: w.get_http_proxy().into(),
            https_proxy: w.get_https_proxy().into(),
            no_proxy: w.get_no_proxy().into(),
            timezone: w.get_timezone().into(),
            locale: w.get_locale().into(),
            keyboard_layout: w.get_keyboard_layout().into(),
            storage_layout: w.get_storage_layout().into(),
            apt_mirror: w.get_apt_mirror().into(),
            packages: w.get_packages().into(),
            apt_repos: w.get_apt_repos().into(),
            dnf_repos: w.get_dnf_repos().into(),
            dnf_mirror: String::new(),
            pacman_repos: String::new(),
            pacman_mirror: String::new(),
            run_commands: w.get_run_commands().into(),
            late_commands: w.get_late_commands().into(),
            firewall_enabled: w.get_firewall_enabled(),
            firewall_policy: w.get_firewall_policy().into(),
            allow_ports: w.get_allow_ports().into(),
            deny_ports: w.get_deny_ports().into(),
            user_groups: w.get_user_groups().into(),
            user_shell: w.get_user_shell().into(),
            sudo_nopasswd: w.get_sudo_nopasswd(),
            sudo_commands: String::new(),
            enable_services: w.get_enable_services().into(),
            disable_services: w.get_disable_services().into(),
            docker: w.get_docker(),
            podman: w.get_podman(),
            docker_users: w.get_docker_users().into(),
            swap_size_mb: w.get_swap_size_mb().into(),
            swap_filename: String::new(),
            swap_swappiness: String::new(),
            encrypt: w.get_encrypt(),
            encrypt_passphrase: w.get_encrypt_passphrase().into(),
            mounts: w.get_mounts().into(),
            grub_timeout: w.get_grub_timeout().into(),
            grub_cmdline: w.get_grub_cmdline().into(),
            grub_default: String::new(),
            sysctl_pairs: w.get_sysctl_pairs().into(),
            no_user_interaction: w.get_no_user_interaction(),
            wallpaper_path: String::new(),
            expected_sha256: w.get_expected_sha256().into(),
        })
    }

    pub fn snap_verify(&self) -> Option<VerifyState> {
        let w = self.win.upgrade()?;
        Some(VerifyState {
            source: w.get_verify_source().into(),
            sums_url: w.get_sums_url().into(),
        })
    }

    fn collect_inject_config(&self) -> Result<(InjectState, InjectConfig), String> {
        let w = self
            .win
            .upgrade()
            .ok_or_else(|| "application window is no longer available".to_string())?;

        let source = w.get_source_path().to_string();
        if source.trim().is_empty() {
            return Err("Source ISO is required".to_string());
        }

        let output_dir = w.get_output_dir().to_string();
        if output_dir.trim().is_empty() {
            return Err("Output directory is required".to_string());
        }
        let output_path = std::path::Path::new(output_dir.trim());
        if !output_path.exists() {
            std::fs::create_dir_all(output_path)
                .map_err(|e| format!("Failed to create output directory: {e}"))?;
        }

        let password: String = w.get_password().into();
        let password_confirm: String = w.get_password_confirm().into();
        if !password.is_empty() && !password_confirm.is_empty() && password != password_confirm {
            return Err("Passwords do not match".to_string());
        }

        let label: String = w.get_output_label().into();
        if !label.is_empty() && label.chars().count() > 32 {
            return Err("Volume label exceeds 32 characters".to_string());
        }

        let inject = self
            .snap_inject()
            .ok_or_else(|| "failed to capture current form state".to_string())?;
        let cfg = build_inject_config(&inject);
        cfg.validate().map_err(|e| format!("Config error: {e}"))?;

        Ok((inject, cfg))
    }

    pub fn validate_inject_form(&self) -> Result<(), String> {
        self.collect_inject_config().map(|_| ())
    }

    // ── Distro defaults ─────────────────────────────────────────────────

    /// Apply distro defaults to unedited fields and update the summary.
    pub fn apply_distro_defaults(&mut self, w: &AppWindow) {
        let distro: String = w.get_distro().into();
        let preset: String = w.get_selected_preset().into();
        let defs = defaults::defaults_for(&distro, &preset);

        let username: String = w.get_username().into();
        let docker_enabled = w.get_docker();
        let changes = defaults::apply_defaults(
            &defs,
            &self.edited_fields,
            &distro,
            &username,
            docker_enabled,
        );

        for (field, value) in &changes {
            match *field {
                "packages" => w.set_packages(value.clone().into()),
                "user_groups" => w.set_user_groups(value.clone().into()),
                "user_shell" => w.set_user_shell(value.clone().into()),
                "enable_services" => w.set_enable_services(value.clone().into()),
                "disable_services" => w.set_disable_services(value.clone().into()),
                "firewall_policy" => w.set_firewall_policy(value.clone().into()),
                "allow_ports" => w.set_allow_ports(value.clone().into()),
                "docker_users" => w.set_docker_users(value.clone().into()),
                _ => {}
            }
        }

        // Update summary display
        let summary = defaults::summary_for(&defs);
        w.set_defaults_summary(summary.into());
    }

    /// Reset edit tracking and reapply all defaults.
    pub fn reset_and_apply_defaults(&mut self, w: &AppWindow) {
        self.edited_fields.clear();
        self.apply_distro_defaults(w);
    }

    /// Mark a field as user-edited so defaults won't overwrite it.
    pub fn mark_edited(&mut self, field: &str) {
        self.edited_fields.insert(field.to_string());
    }

    pub fn clear_defaults_state(&mut self) {
        self.edited_fields.clear();
    }

    pub fn seed_default_edit_tracking(&mut self, w: &AppWindow) {
        let distro: String = w.get_distro().into();
        let preset: String = w.get_selected_preset().into();
        let defs = defaults::defaults_for(&distro, &preset);
        let username: String = w.get_username().into();
        let docker_enabled = w.get_docker();

        let expected =
            defaults::apply_defaults(&defs, &HashSet::new(), &distro, &username, docker_enabled);

        self.edited_fields.clear();

        for (field, value) in expected {
            let current = match field {
                "packages" => w.get_packages().to_string(),
                "user_groups" => w.get_user_groups().to_string(),
                "user_shell" => w.get_user_shell().to_string(),
                "enable_services" => w.get_enable_services().to_string(),
                "disable_services" => w.get_disable_services().to_string(),
                "firewall_policy" => w.get_firewall_policy().to_string(),
                "allow_ports" => w.get_allow_ports().to_string(),
                "docker_users" => w.get_docker_users().to_string(),
                _ => continue,
            };

            if current.trim() != value.trim() {
                self.edited_fields.insert(field.to_string());
            }
        }
    }

    fn sync_auto_managed_access(&self, w: &AppWindow) {
        let username: String = w.get_username().into();
        let distro: String = w.get_distro().into();

        if !self.edited_fields.contains("user_groups") {
            let groups = defaults::auto_user_groups(&distro, &username);
            w.set_user_groups(groups.into());
        }

        if !self.edited_fields.contains("docker_users") {
            if w.get_docker() && !username.is_empty() {
                w.set_docker_users(username.into());
            } else {
                w.set_docker_users("".into());
            }
        }
    }

    /// Auto-manage user groups and Docker users when the username changes.
    /// Only modifies fields that the user hasn't manually edited.
    pub fn on_username_changed(&mut self, w: &AppWindow) {
        self.sync_auto_managed_access(w);
    }

    /// Recompute Docker user defaults when Docker is toggled on or off.
    pub fn on_docker_changed(&mut self, w: &AppWindow) {
        self.sync_auto_managed_access(w);
    }

    // ── spawn_inject ──────────────────────────────────────────────────────────

    pub fn spawn_inject(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };

        if w.get_job_running() {
            return;
        }

        let (inject, cfg) = match self.collect_inject_config() {
            Ok(values) => values,
            Err(msg) => {
                self.set_status_err(msg);
                return;
            }
        };

        // Abort stale tasks
        if let Some(h) = self.detect_task.take() {
            h.abort();
        }
        if let Some(h) = self.sha256_task.take() {
            h.abort();
        }

        // Reset build/check state while preserving completed configuration.
        clear_build_results(&w);

        self.start_job("Injecting…");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();
        let out_dir = PathBuf::from(&inject.output_dir);

        self.current_task = Some(self.rt.spawn(async move {
            match engine.inject_autoinstall(&cfg, &out_dir).await {
                Ok(result) => {
                    let artifact = result
                        .artifacts
                        .first()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    let art2 = artifact.clone();
                    let win3 = win2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            w.set_job_running(false);
                            w.set_job_phase("".into());
                            w.set_step3_done(true);
                            w.set_artifact_path(artifact.clone().into());
                            w.set_verify_source(artifact.into());
                            w.set_current_step(3);
                            w.set_status_text(
                                "ISO ready — optional checks are available if you want them".into(),
                            );
                            w.set_status_is_error(false);
                        }
                    });

                    // Background SHA-256
                    if !art2.is_empty() {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win3.upgrade() {
                                spawn_sha256(w.as_weak(), art2);
                            }
                        });
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            w.set_job_running(false);
                            w.set_job_phase("".into());
                            w.set_status_text(
                                msg.as_str().chars().take(200).collect::<String>().into(),
                            );
                            w.set_status_is_error(true);
                        }
                    });
                }
            }
        }));
    }

    // ── spawn_verify ──────────────────────────────────────────────────────────

    pub fn spawn_verify(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };
        if w.get_job_running() {
            return;
        }
        // Abort any previous verify
        if let Some(h) = self.current_task.take() {
            h.abort();
        }

        let source: String = w.get_verify_source().into();
        if source.trim().is_empty() {
            self.set_status_err("Source ISO is required for verification");
            return;
        }
        let sums: String = w.get_sums_url().into();
        let sums_opt = opt(&sums);

        w.set_verify_done(false);
        w.set_verify_matched(false);
        w.set_verify_hash_display("".into());
        self.start_job("Verifying checksum…");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.current_task = Some(self.rt.spawn(async move {
            match engine.verify(&source, sums_opt.as_deref()).await {
                Ok(r) => {
                    let matched = r.matched;
                    let hash = r.actual.clone();
                    let display = format!(
                        "{}{}",
                        if matched {
                            "Match: "
                        } else {
                            "Mismatch — actual: "
                        },
                        hash
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            w.set_job_running(false);
                            w.set_job_phase("".into());
                            w.set_verify_done(true);
                            w.set_verify_matched(matched);
                            w.set_verify_hash_display(display.into());
                            w.set_step3_done(true);
                            w.set_status_text(
                                if matched {
                                    "Integrity verified ✓"
                                } else {
                                    "Checksum mismatch"
                                }
                                .into(),
                            );
                            w.set_status_is_error(!matched);
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            w.set_job_running(false);
                            w.set_job_phase("".into());
                            w.set_status_text(
                                msg.as_str().chars().take(200).collect::<String>().into(),
                            );
                            w.set_status_is_error(true);
                        }
                    });
                }
            }
        }));
    }

    // ── spawn_iso9660 ─────────────────────────────────────────────────────────

    pub fn spawn_iso9660(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };
        if w.get_job_running() {
            return;
        }
        if let Some(h) = self.current_task.take() {
            h.abort();
        }

        let source: String = w.get_verify_source().into();
        if source.trim().is_empty() {
            self.set_status_err("Source ISO is required");
            return;
        }
        w.set_iso9660_done(false);
        w.set_iso9660_compliant(false);
        w.set_iso9660_boot_bios(false);
        w.set_iso9660_boot_uefi(false);
        w.set_iso9660_volume_id("".into());
        self.start_job("Validating ISO-9660…");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.current_task = Some(self.rt.spawn(async move {
            match engine.validate_iso9660(&source).await {
                Ok(r) => {
                    let compliant = r.compliant;
                    let bios = r.boot_bios;
                    let uefi = r.boot_uefi;
                    let vol = r.volume_id.clone().unwrap_or_default();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            w.set_job_running(false);
                            w.set_job_phase("".into());
                            w.set_iso9660_done(true);
                            w.set_iso9660_compliant(compliant);
                            w.set_iso9660_boot_bios(bios);
                            w.set_iso9660_boot_uefi(uefi);
                            w.set_iso9660_volume_id(vol.into());
                            w.set_status_text(
                                if compliant {
                                    "ISO-9660 compliant ✓"
                                } else {
                                    "ISO-9660 issues found"
                                }
                                .into(),
                            );
                            w.set_status_is_error(!compliant);
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            w.set_job_running(false);
                            w.set_job_phase("".into());
                            w.set_status_text(
                                msg.as_str().chars().take(200).collect::<String>().into(),
                            );
                            w.set_status_is_error(true);
                        }
                    });
                }
            }
        }));
    }

    // ── spawn_doctor ──────────────────────────────────────────────────────────

    pub fn spawn_doctor(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };
        if w.get_job_running() {
            return;
        }
        if let Some(h) = self.current_task.take() {
            h.abort();
        }

        w.set_doctor_loading(true);
        w.set_doctor_text("".into());
        self.start_job("Checking dependencies…");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.current_task = Some(self.rt.spawn(async move {
            let report = engine.doctor().await;
            // Build a human-readable text from BTreeMap<tool, ok>.
            let mut lines_out = Vec::new();
            for (tool, ok) in &report.tooling {
                let mark = if *ok { "✓" } else { "✗" };
                lines_out.push(format!("  {mark}  {tool}"));
            }
            if !report.warnings.is_empty() {
                lines_out.push(String::new());
                for w in &report.warnings {
                    lines_out.push(format!("  ⚠  {w}"));
                }
            }
            let text = lines_out.join("\n");
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win2.upgrade() {
                    w.set_job_running(false);
                    w.set_job_phase("".into());
                    w.set_doctor_loading(false);
                    w.set_doctor_text(text.into());
                }
            });
        }));
    }

    // ── spawn_detect_iso ──────────────────────────────────────────────────────

    pub fn spawn_detect_iso(&mut self, path: String) {
        // Abort any existing detection
        if let Some(h) = self.detect_task.take() {
            h.abort();
        }

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.detect_task = Some(self.rt.spawn(async move {
            if let Ok(meta) = engine.inspect_source(&path, None).await {
                let distro_str = match meta.distro {
                    Some(Distro::Ubuntu) => "ubuntu",
                    Some(Distro::Fedora) => "fedora",
                    Some(Distro::Arch) => "arch",
                    Some(Distro::Mint) => "mint",
                    _ => "",
                };
                let label = meta.volume_id.clone().unwrap_or_default();
                let distro = distro_str.to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win2.upgrade() {
                        if !distro.is_empty() {
                            w.set_distro(distro.clone().into());
                            w.set_detected_distro(
                                match distro.as_str() {
                                    "fedora" => "Fedora / RHEL",
                                    "arch" => "Arch Linux",
                                    "mint" => "Linux Mint",
                                    _ => "Ubuntu",
                                }
                                .into(),
                            );
                        }
                        if !label.is_empty() && w.get_output_label().is_empty() {
                            w.set_output_label(label.into());
                        }
                    }
                });
            }
        }));
    }
}

// ── SHA-256 background task ───────────────────────────────────────────────────

pub fn spawn_sha256(win: Weak<AppWindow>, path: String) {
    std::thread::spawn(move || {
        let hash = compute_sha256(&path);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = win.upgrade() {
                w.set_artifact_sha256(hash.into());
            }
        });
    });
}

fn compute_sha256(path: &str) -> String {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    if std::io::copy(&mut file, &mut hasher).is_err() {
        return String::new();
    }
    format!("{:x}", hasher.finalize())
}

// ── Preset selection handler ──────────────────────────────────────────────────

pub fn handle_preset_clicked(w: &AppWindow, id: &str, app: &mut ForgeApp) {
    if let Some(p) = find_preset_by_str(id) {
        w.set_selected_preset(p.id.as_str().into());
        w.set_selected_preset_name(p.name.into());
        w.set_distro(p.distro.into());
        // Clear stale build state
        w.set_step2_done(false);
        clear_build_results(w);

        if p.strategy == AcquisitionStrategy::DirectUrl {
            if let Ok(Some(url)) = resolve_url(p) {
                w.set_source_path(url.into());
                // Trigger ISO detection on URL-resolved presets
                app.spawn_detect_iso(w.get_source_path().into());
            }
        }

        // Apply distro-aware defaults for this preset.
        app.apply_distro_defaults(w);

        w.set_current_step(2);
    }
}

// ── InjectConfig builder ──────────────────────────────────────────────────────

pub fn build_inject_config(s: &InjectState) -> InjectConfig {
    let source = IsoSource::from_raw(s.source.trim());
    let shared_repo_lines = lines(&s.apt_repos);

    let distro = match s.distro.as_str() {
        "fedora" => Some(Distro::Fedora),
        "arch" => Some(Distro::Arch),
        "mint" => Some(Distro::Mint),
        _ => None,
    };

    let ssh = SshConfig {
        authorized_keys: lines(&s.ssh_keys),
        install_server: Some(s.ssh_install_server),
        allow_password_auth: Some(s.ssh_password_auth),
    };

    let network = NetworkConfig {
        dns_servers: tokens(&s.dns_servers),
        ntp_servers: tokens(&s.ntp_servers),
    };

    let proxy = ProxyConfig {
        http_proxy: opt(&s.http_proxy),
        https_proxy: opt(&s.https_proxy),
        no_proxy: tokens(&s.no_proxy),
    };

    let user = UserConfig {
        groups: lines(&s.user_groups),
        shell: opt(&s.user_shell),
        sudo_nopasswd: s.sudo_nopasswd,
        sudo_commands: lines(&s.sudo_commands),
    };

    let firewall = FirewallConfig {
        enabled: s.firewall_enabled,
        default_policy: opt(&s.firewall_policy),
        allow_ports: tokens(&s.allow_ports),
        deny_ports: tokens(&s.deny_ports),
    };

    let swap = s
        .swap_size_mb
        .parse::<u32>()
        .ok()
        .map(|size_mb| SwapConfig {
            size_mb,
            filename: opt(&s.swap_filename),
            swappiness: s.swap_swappiness.parse::<u8>().ok(),
        });

    let containers = ContainerConfig {
        docker: s.docker,
        podman: s.podman,
        docker_users: lines(&s.docker_users),
    };

    let grub = GrubConfig {
        timeout: s.grub_timeout.parse::<u32>().ok(),
        cmdline_extra: tokens(&s.grub_cmdline),
        default_entry: opt(&s.grub_default),
    };

    let sysctl: Vec<(String, String)> = s
        .sysctl_pairs
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let mut parts = l.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) => {
                    let k = k.trim().to_string();
                    let v = v.trim().to_string();
                    if k.is_empty() || v.is_empty() {
                        None
                    } else {
                        Some((k, v))
                    }
                }
                _ => None,
            }
        })
        .collect();

    InjectConfig {
        source,
        autoinstall_yaml: None,
        out_name: s.out_name.clone(),
        output_label: opt(&s.output_label),
        expected_sha256: opt(&s.expected_sha256),
        hostname: opt(&s.hostname),
        username: opt(&s.username),
        password: opt(&s.password),
        realname: opt(&s.realname),
        ssh,
        network,
        proxy,
        user,
        timezone: opt(&s.timezone),
        locale: opt(&s.locale),
        keyboard_layout: opt(&s.keyboard_layout),
        storage_layout: opt(&s.storage_layout),
        apt_mirror: opt(&s.apt_mirror),
        extra_packages: tokens(&s.packages),
        wallpaper: opt(&s.wallpaper_path).map(PathBuf::from),
        extra_late_commands: lines(&s.late_commands),
        no_user_interaction: s.no_user_interaction,
        firewall,
        static_ip: opt(&s.static_ip),
        gateway: opt(&s.gateway),
        enable_services: lines(&s.enable_services),
        disable_services: lines(&s.disable_services),
        sysctl,
        swap,
        apt_repos: if matches!(distro, None | Some(Distro::Mint)) {
            shared_repo_lines.clone()
        } else {
            Vec::new()
        },
        dnf_repos: if matches!(distro, Some(Distro::Fedora)) {
            let repos = lines(&s.dnf_repos);
            if repos.is_empty() {
                shared_repo_lines.clone()
            } else {
                repos
            }
        } else {
            Vec::new()
        },
        dnf_mirror: opt(&s.dnf_mirror),
        pacman_repos: if matches!(distro, Some(Distro::Arch)) {
            let repos = lines(&s.pacman_repos);
            if repos.is_empty() {
                shared_repo_lines.clone()
            } else {
                repos
            }
        } else {
            Vec::new()
        },
        pacman_mirror: opt(&s.pacman_mirror),
        containers,
        grub,
        encrypt: s.encrypt,
        encrypt_passphrase: opt(&s.encrypt_passphrase),
        mounts: lines(&s.mounts),
        run_commands: lines(&s.run_commands),
        distro,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_inject_config, preset_display_name};
    use crate::state::InjectState;

    #[test]
    fn preset_display_name_uses_engine_metadata() {
        assert_eq!(
            preset_display_name("ubuntu-server-lts"),
            Some("Ubuntu 24.04.4 LTS Server")
        );
        assert_eq!(preset_display_name("arch-linux"), Some("Arch Linux"));
    }

    #[test]
    fn preset_display_name_rejects_unknown_ids() {
        assert_eq!(preset_display_name("unknown-preset"), None);
    }

    #[test]
    fn build_inject_config_splits_flexible_token_fields() {
        let state = InjectState {
            dns_servers: "1.1.1.1, 8.8.8.8".into(),
            ntp_servers: "time1.example.com\ntime2.example.com".into(),
            no_proxy: "localhost,127.0.0.1 internal.example.com".into(),
            packages: "curl git\nhtop".into(),
            allow_ports: "22 80/tcp".into(),
            deny_ports: "23,25".into(),
            grub_cmdline: "quiet splash".into(),
            ..InjectState::default()
        };

        let cfg = build_inject_config(&state);
        assert_eq!(cfg.network.dns_servers, vec!["1.1.1.1", "8.8.8.8"]);
        assert_eq!(
            cfg.network.ntp_servers,
            vec!["time1.example.com", "time2.example.com"]
        );
        assert_eq!(
            cfg.proxy.no_proxy,
            vec!["localhost", "127.0.0.1", "internal.example.com"]
        );
        assert_eq!(cfg.extra_packages, vec!["curl", "git", "htop"]);
        assert_eq!(cfg.firewall.allow_ports, vec!["22", "80/tcp"]);
        assert_eq!(cfg.firewall.deny_ports, vec!["23", "25"]);
        assert_eq!(cfg.grub.cmdline_extra, vec!["quiet", "splash"]);
    }
}
