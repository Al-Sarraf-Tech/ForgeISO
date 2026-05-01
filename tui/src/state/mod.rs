//! TUI application state.
//!
//! Public surface (preserved from the previous single-file `state.rs`):
//! * [`App`] — the entire wizard state struct.
//! * [`LogEntry`], [`LogLevel`] — log pane entries.
//! * [`SourceFocus`], [`WizardStep`], [`ConfigTab`] — navigation enums.
//! * [`WorkerMsg`] — async worker → UI message type.
//! * [`FieldDef`], [`FieldKind`] — Configure-tab form descriptors.
//!
//! Internal layout:
//! * [`nav`] — navigation enums and worker message.
//! * [`fields`] — `FieldDef` / `FieldKind` form descriptors.
//! * [`source`] — Step 1 (Source) helpers on `App`.
//! * [`configure`] — Step 2 (Configure) form schema, validation, and
//!   `InjectConfig` builder on `App`.
//! * [`build`] — Step 3 (Build) helpers and shared invalidation on `App`.
//!
//! All items below are re-exported at `crate::state::*` so existing
//! `use crate::state::{...}` paths in `main.rs`, `ui::*`, and `worker.rs`
//! continue to compile unchanged.

mod build;
mod configure;
mod fields;
mod nav;
mod source;

pub(crate) use fields::{FieldDef, FieldKind};
pub(crate) use nav::{ConfigTab, LogEntry, LogLevel, SourceFocus, WizardStep, WorkerMsg};

use std::path::PathBuf;

use forgeiso_engine::{GuidedWorkflowProgress, Iso9660Compliance, VerifyResult};

pub(crate) struct App {
    // Navigation
    pub(crate) step: WizardStep,
    pub(crate) progress: GuidedWorkflowProgress,

    // Step 1: Source
    pub(crate) source_focus: SourceFocus,
    pub(crate) preset_scroll: usize,
    pub(crate) preset_selected: Option<usize>,
    pub(crate) manual_source: String,
    pub(crate) detected_distro: Option<String>,

    // Step 2: Configure
    pub(crate) config_tab: ConfigTab,
    pub(crate) field_index: usize,
    pub(crate) editing: bool,

    // Form fields — Identity
    pub(crate) hostname: String,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) password_confirm: String,
    pub(crate) realname: String,
    pub(crate) distro: String,

    // SSH
    pub(crate) ssh_keys: String,
    pub(crate) ssh_password_auth: bool,
    pub(crate) ssh_install_server: bool,

    // Network
    pub(crate) dns_servers: String,
    pub(crate) ntp_servers: String,
    pub(crate) static_ip: String,
    pub(crate) gateway: String,
    pub(crate) http_proxy: String,
    pub(crate) https_proxy: String,
    pub(crate) no_proxy: String,

    // Packages
    pub(crate) packages: String,
    pub(crate) apt_repos: String,
    pub(crate) dnf_repos: String,
    pub(crate) apt_mirror: String,

    // Services
    pub(crate) enable_services: String,
    pub(crate) disable_services: String,
    pub(crate) docker: bool,
    pub(crate) podman: bool,
    pub(crate) docker_users: String,
    pub(crate) firewall_enabled: bool,
    pub(crate) firewall_policy: String,
    pub(crate) allow_ports: String,
    pub(crate) deny_ports: String,

    // Advanced
    pub(crate) timezone: String,
    pub(crate) locale: String,
    pub(crate) keyboard_layout: String,
    pub(crate) storage_layout: String,
    pub(crate) run_commands: String,
    pub(crate) late_commands: String,
    pub(crate) sysctl_pairs: String,
    pub(crate) encrypt: bool,
    pub(crate) encrypt_passphrase: String,
    pub(crate) swap_size_mb: String,
    pub(crate) grub_timeout: String,
    pub(crate) grub_cmdline: String,
    pub(crate) mounts: String,
    pub(crate) no_user_interaction: bool,
    pub(crate) user_groups: String,
    pub(crate) user_shell: String,
    pub(crate) sudo_nopasswd: bool,

    // Output
    pub(crate) output_dir: String,
    pub(crate) out_name: String,
    pub(crate) output_label: String,
    pub(crate) expected_sha256: String,

    // Step 3: Build
    pub(crate) busy: bool,
    pub(crate) build_artifact: Option<PathBuf>,
    pub(crate) build_sha256: Option<String>,

    // Step 4: Optional checks
    pub(crate) verify_source: String,
    pub(crate) verify_result: Option<VerifyResult>,
    pub(crate) iso9660_result: Option<Iso9660Compliance>,
    pub(crate) check_field_index: usize,
    pub(crate) check_editing: bool,

    // Shared
    pub(crate) status: String,
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) log_scroll: usize,
    pub(crate) quit_confirm: bool,
}

impl App {
    pub(crate) fn new(doctor: forgeiso_engine::DoctorReport) -> Self {
        let mut logs = Vec::new();
        logs.push(LogEntry {
            text: format!(
                "doctor: host={} arch={} linux={}",
                doctor.host_os, doctor.host_arch, doctor.linux_supported
            ),
            level: LogLevel::Info,
        });
        for (tool, available) in &doctor.tooling {
            let level = if *available {
                LogLevel::Info
            } else {
                LogLevel::Warn
            };
            logs.push(LogEntry {
                text: format!("  {tool}: {}", if *available { "ok" } else { "missing" }),
                level,
            });
        }

        Self {
            step: WizardStep::Source,
            progress: GuidedWorkflowProgress::default(),

            source_focus: SourceFocus::PresetList,
            preset_scroll: 0,
            preset_selected: None,
            manual_source: String::new(),
            detected_distro: None,

            config_tab: ConfigTab::Identity,
            field_index: 0,
            editing: false,

            hostname: String::new(),
            username: String::new(),
            password: String::new(),
            password_confirm: String::new(),
            realname: String::new(),
            distro: String::new(),

            ssh_keys: String::new(),
            ssh_password_auth: true,
            ssh_install_server: true,

            dns_servers: String::new(),
            ntp_servers: String::new(),
            static_ip: String::new(),
            gateway: String::new(),
            http_proxy: String::new(),
            https_proxy: String::new(),
            no_proxy: String::new(),

            packages: String::new(),
            apt_repos: String::new(),
            dnf_repos: String::new(),
            apt_mirror: String::new(),

            enable_services: String::new(),
            disable_services: String::new(),
            docker: false,
            podman: false,
            docker_users: String::new(),
            firewall_enabled: false,
            firewall_policy: String::new(),
            allow_ports: String::new(),
            deny_ports: String::new(),

            timezone: String::new(),
            locale: String::new(),
            keyboard_layout: String::new(),
            storage_layout: String::new(),
            run_commands: String::new(),
            late_commands: String::new(),
            sysctl_pairs: String::new(),
            encrypt: false,
            encrypt_passphrase: String::new(),
            swap_size_mb: String::new(),
            grub_timeout: String::new(),
            grub_cmdline: String::new(),
            mounts: String::new(),
            no_user_interaction: false,
            user_groups: String::new(),
            user_shell: String::new(),
            sudo_nopasswd: false,

            output_dir: "/tmp/forgeoutput".to_string(),
            out_name: "forgeiso-custom".to_string(),
            output_label: String::new(),
            expected_sha256: String::new(),

            busy: false,
            build_artifact: None,
            build_sha256: None,

            verify_source: String::new(),
            verify_result: None,
            iso9660_result: None,
            check_field_index: 0,
            check_editing: false,

            status: "Ready".into(),
            logs,
            log_scroll: 0,
            quit_confirm: false,
        }
    }

    pub(crate) fn push_log(&mut self, text: String, level: LogLevel) {
        self.logs.push(LogEntry { text, level });
        // Keep scrolled to bottom when new entries arrive.
        let max = self.logs.len().saturating_sub(8);
        if self.log_scroll < max {
            self.log_scroll = max;
        }
    }
}
