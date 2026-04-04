use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use forgeiso_engine::{ForgeIsoEngine, InjectConfig};
use slint::{ComponentHandle, Model, ModelRc, VecModel, Weak};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use std::collections::HashSet;

use crate::config::build_inject_config;
use crate::defaults;
use crate::state::{InjectState, VerifyState};
use crate::{AppState, AppWindow, FormState, LogEntry};

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

// ── Log helpers ──────────────────────────────────────────────────────────────

const MAX_LOG: usize = 5_000;

fn now_ts() -> slint::SharedString {
    chrono::Local::now().format("%H:%M:%S").to_string().into()
}

// ── Main app state ───────────────────────────────────────────────────────────

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

    // ── Job lifecycle ────────────────────────────────────────────────────────

    pub(crate) fn start_job(&mut self, phase: &str) {
        self.download_idx.clear();
        if let Some(w) = self.win.upgrade() {
            let gs = w.global::<AppState>();
            gs.set_job_running(true);
            gs.set_job_phase(phase.into());
            gs.set_job_percent(-1.0);
            gs.set_status_text("".into());
        }
    }

    pub fn finish_job(&self) {
        if let Some(w) = self.win.upgrade() {
            let gs = w.global::<AppState>();
            gs.set_job_running(false);
            gs.set_job_phase("".into());
            gs.set_job_percent(-1.0);
        }
    }

    pub fn set_status_ok(&self, msg: impl Into<String>) {
        if let Some(w) = self.win.upgrade() {
            let gs = w.global::<AppState>();
            gs.set_status_text(msg.into().into());
            gs.set_status_is_error(false);
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
            let gs = w.global::<AppState>();
            gs.set_status_text(first.into());
            gs.set_status_is_error(true);
        }
    }

    pub fn push_log(&mut self, phase: &str, message: &str, level: i32, percent: Option<f32>) {
        let m = &self.log_vec;
        // Engine reports percent as 0–100; UI progress bars expect 0.0–1.0.
        let pct_val = match percent {
            Some(p) if p >= 0.0 => (p / 100.0).clamp(0.0, 1.0),
            _ => -1.0,
        };

        // For download progress: update in-place rather than append.
        if pct_val >= 0.0 {
            if let Some(&idx) = self.download_idx.get(phase) {
                if idx < m.row_count() {
                    if let Some(mut entry) = m.row_data(idx) {
                        entry.message = message.into();
                        entry.percent = pct_val;
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
            percent: pct_val,
        });

        // Track download phase index for in-place progress updates.
        if pct_val >= 0.0 {
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
                let gs = w.global::<AppState>();
                gs.set_log_error_count(gs.get_log_error_count() + 1);
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

    // ── Snapshot current form state from Slint ───────────────────────────────

    pub fn snap_inject(&self) -> Option<InjectState> {
        let w = self.win.upgrade()?;
        let fs = w.global::<FormState>();
        Some(InjectState {
            source: fs.get_source_path().into(),
            source_preset: fs.get_selected_preset().into(),
            output_dir: fs.get_output_dir().into(),
            out_name: fs.get_out_name().into(),
            output_label: fs.get_output_label().into(),
            distro: fs.get_distro().into(),
            hostname: fs.get_hostname().into(),
            username: fs.get_username().into(),
            password: fs.get_password().into(),
            password_confirm: fs.get_password_confirm().into(),
            realname: fs.get_realname().into(),
            ssh_keys: fs.get_ssh_keys().into(),
            ssh_password_auth: fs.get_ssh_password_auth(),
            ssh_install_server: fs.get_ssh_install_server(),
            dns_servers: fs.get_dns_servers().into(),
            ntp_servers: fs.get_ntp_servers().into(),
            static_ip: fs.get_static_ip().into(),
            gateway: fs.get_gateway().into(),
            http_proxy: fs.get_http_proxy().into(),
            https_proxy: fs.get_https_proxy().into(),
            no_proxy: fs.get_no_proxy().into(),
            timezone: fs.get_timezone().into(),
            locale: fs.get_locale().into(),
            keyboard_layout: fs.get_keyboard_layout().into(),
            storage_layout: fs.get_storage_layout().into(),
            apt_mirror: fs.get_apt_mirror().into(),
            packages: fs.get_packages().into(),
            apt_repos: fs.get_apt_repos().into(),
            dnf_repos: fs.get_dnf_repos().into(),
            dnf_mirror: fs.get_dnf_mirror().into(),
            pacman_repos: fs.get_pacman_repos().into(),
            pacman_mirror: fs.get_pacman_mirror().into(),
            run_commands: fs.get_run_commands().into(),
            late_commands: fs.get_late_commands().into(),
            firewall_enabled: fs.get_firewall_enabled(),
            firewall_policy: fs.get_firewall_policy().into(),
            allow_ports: fs.get_allow_ports().into(),
            deny_ports: fs.get_deny_ports().into(),
            user_groups: fs.get_user_groups().into(),
            user_shell: fs.get_user_shell().into(),
            sudo_nopasswd: fs.get_sudo_nopasswd(),
            sudo_commands: fs.get_sudo_commands().into(),
            enable_services: fs.get_enable_services().into(),
            disable_services: fs.get_disable_services().into(),
            docker: fs.get_docker(),
            podman: fs.get_podman(),
            docker_users: fs.get_docker_users().into(),
            swap_size_mb: fs.get_swap_size_mb().into(),
            swap_filename: fs.get_swap_filename().into(),
            swap_swappiness: fs.get_swap_swappiness().into(),
            encrypt: fs.get_encrypt(),
            encrypt_passphrase: fs.get_encrypt_passphrase().into(),
            mounts: fs.get_mounts().into(),
            grub_timeout: fs.get_grub_timeout().into(),
            grub_cmdline: fs.get_grub_cmdline().into(),
            grub_default: fs.get_grub_default().into(),
            sysctl_pairs: fs.get_sysctl_pairs().into(),
            no_user_interaction: fs.get_no_user_interaction(),
            wallpaper_path: fs.get_wallpaper_path().into(),
            expected_sha256: fs.get_expected_sha256().into(),
        })
    }

    pub fn snap_verify(&self) -> Option<VerifyState> {
        let w = self.win.upgrade()?;
        let gs = w.global::<AppState>();
        Some(VerifyState {
            source: gs.get_verify_source().into(),
            sums_url: gs.get_sums_url().into(),
        })
    }

    pub(crate) fn collect_inject_config(&self) -> Result<(InjectState, InjectConfig), String> {
        let w = self
            .win
            .upgrade()
            .ok_or_else(|| "application window is no longer available".to_string())?;

        let fs = w.global::<FormState>();
        let source = fs.get_source_path().to_string();
        if source.trim().is_empty() {
            return Err("Source ISO is required".to_string());
        }

        let output_dir = fs.get_output_dir().to_string();
        if output_dir.trim().is_empty() {
            return Err("Output directory is required".to_string());
        }
        let output_path = std::path::Path::new(output_dir.trim());
        if !output_path.exists() {
            std::fs::create_dir_all(output_path)
                .map_err(|e| format!("Failed to create output directory: {e}"))?;
        }

        let password: String = fs.get_password().into();
        let password_confirm: String = fs.get_password_confirm().into();
        if !password.is_empty() && !password_confirm.is_empty() && password != password_confirm {
            return Err("Passwords do not match".to_string());
        }

        let label: String = fs.get_output_label().into();
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

    // ── Distro defaults ─────────────────────────────────────────────────────

    /// Apply distro defaults to unedited fields and update the summary.
    pub fn apply_distro_defaults(&mut self, w: &AppWindow) {
        let fs = w.global::<FormState>();
        let distro: String = fs.get_distro().into();
        let preset: String = fs.get_selected_preset().into();
        let defs = defaults::defaults_for(&distro, &preset);

        let username: String = fs.get_username().into();
        let docker_enabled = fs.get_docker();
        let changes = defaults::apply_defaults(
            &defs,
            &self.edited_fields,
            &distro,
            &username,
            docker_enabled,
        );

        for (field, value) in &changes {
            match *field {
                "packages" => fs.set_packages(value.clone().into()),
                "user_groups" => fs.set_user_groups(value.clone().into()),
                "user_shell" => fs.set_user_shell(value.clone().into()),
                "enable_services" => fs.set_enable_services(value.clone().into()),
                "disable_services" => fs.set_disable_services(value.clone().into()),
                "firewall_policy" => fs.set_firewall_policy(value.clone().into()),
                "allow_ports" => fs.set_allow_ports(value.clone().into()),
                "docker_users" => fs.set_docker_users(value.clone().into()),
                _ => {}
            }
        }

        // Update summary display
        let summary = defaults::summary_for(&defs);
        w.global::<AppState>().set_defaults_summary(summary.into());
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
        let fs = w.global::<FormState>();
        let distro: String = fs.get_distro().into();
        let preset: String = fs.get_selected_preset().into();
        let defs = defaults::defaults_for(&distro, &preset);
        let username: String = fs.get_username().into();
        let docker_enabled = fs.get_docker();

        let expected =
            defaults::apply_defaults(&defs, &HashSet::new(), &distro, &username, docker_enabled);

        self.edited_fields.clear();

        for (field, value) in expected {
            let current = match field {
                "packages" => fs.get_packages().to_string(),
                "user_groups" => fs.get_user_groups().to_string(),
                "user_shell" => fs.get_user_shell().to_string(),
                "enable_services" => fs.get_enable_services().to_string(),
                "disable_services" => fs.get_disable_services().to_string(),
                "firewall_policy" => fs.get_firewall_policy().to_string(),
                "allow_ports" => fs.get_allow_ports().to_string(),
                "docker_users" => fs.get_docker_users().to_string(),
                _ => continue,
            };

            if current.trim() != value.trim() {
                self.edited_fields.insert(field.to_string());
            }
        }
    }

    fn sync_auto_managed_access(&self, w: &AppWindow) {
        let fs = w.global::<FormState>();
        let username: String = fs.get_username().into();
        let distro: String = fs.get_distro().into();

        if !self.edited_fields.contains("user_groups") {
            let groups = defaults::auto_user_groups(&distro, &username);
            fs.set_user_groups(groups.into());
        }

        if !self.edited_fields.contains("docker_users") {
            if fs.get_docker() && !username.is_empty() {
                fs.set_docker_users(username.into());
            } else {
                fs.set_docker_users("".into());
            }
        }
    }

    /// Auto-manage user groups and Docker users when the username changes.
    pub fn on_username_changed(&mut self, w: &AppWindow) {
        self.sync_auto_managed_access(w);
    }

    /// Recompute Docker user defaults when Docker is toggled on or off.
    pub fn on_docker_changed(&mut self, w: &AppWindow) {
        self.sync_auto_managed_access(w);
    }
}
