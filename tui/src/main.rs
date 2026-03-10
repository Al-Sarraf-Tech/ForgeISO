use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use forgeiso_engine::{
    all_presets, BuildResult, ContainerConfig, Distro, FirewallConfig, ForgeIsoEngine, GrubConfig,
    GuidedWorkflowProgress, GuidedWorkflowStep, InjectConfig, Iso9660Compliance, IsoMetadata,
    IsoPreset, IsoSource, NetworkConfig, ProxyConfig, SshConfig, SwapConfig, UserConfig,
    VerifyResult,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Terminal,
};
use tokio::sync::mpsc;

// ─── Worker messages ────────────────────────────────────────────────────────

#[allow(dead_code)]
enum WorkerMsg {
    InspectOk(Box<IsoMetadata>),
    InjectOk(Box<BuildResult>),
    EngineEvent(String, LogLevel),
    VerifyOk(Box<VerifyResult>),
    Iso9660Ok(Box<Iso9660Compliance>),
    OpError(String),
}

// ─── Types ──────────────────────────────────────────────────────────────────

type WizardStep = GuidedWorkflowStep;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigTab {
    Identity,
    Network,
    Packages,
    Services,
    Advanced,
    Output,
}

impl ConfigTab {
    const ALL: [Self; 6] = [
        Self::Identity,
        Self::Network,
        Self::Packages,
        Self::Services,
        Self::Advanced,
        Self::Output,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Network => "Network",
            Self::Packages => "Packages",
            Self::Services => "Services",
            Self::Advanced => "Advanced",
            Self::Output => "Output",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceFocus {
    PresetList,
    ManualInput,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Info,
    Warn,
    Error,
}

struct LogEntry {
    text: String,
    level: LogLevel,
}

// ─── App state ──────────────────────────────────────────────────────────────

struct App {
    // Navigation
    step: WizardStep,
    progress: GuidedWorkflowProgress,

    // Step 1: Source
    source_focus: SourceFocus,
    preset_scroll: usize,
    preset_selected: Option<usize>,
    manual_source: String,
    detected_distro: Option<String>,

    // Step 2: Configure
    config_tab: ConfigTab,
    field_index: usize,
    editing: bool,

    // Form fields — Identity
    hostname: String,
    username: String,
    password: String,
    password_confirm: String,
    realname: String,
    distro: String,

    // SSH
    ssh_keys: String,
    ssh_password_auth: bool,
    ssh_install_server: bool,

    // Network
    dns_servers: String,
    ntp_servers: String,
    static_ip: String,
    gateway: String,
    http_proxy: String,
    https_proxy: String,
    no_proxy: String,

    // Packages
    packages: String,
    apt_repos: String,
    dnf_repos: String,
    apt_mirror: String,

    // Services
    enable_services: String,
    disable_services: String,
    docker: bool,
    podman: bool,
    docker_users: String,
    firewall_enabled: bool,
    firewall_policy: String,
    allow_ports: String,
    deny_ports: String,

    // Advanced
    timezone: String,
    locale: String,
    keyboard_layout: String,
    storage_layout: String,
    run_commands: String,
    late_commands: String,
    sysctl_pairs: String,
    encrypt: bool,
    encrypt_passphrase: String,
    swap_size_mb: String,
    grub_timeout: String,
    grub_cmdline: String,
    mounts: String,
    no_user_interaction: bool,
    user_groups: String,
    user_shell: String,
    sudo_nopasswd: bool,

    // Output
    output_dir: String,
    out_name: String,
    output_label: String,
    expected_sha256: String,

    // Step 3: Build
    busy: bool,
    build_artifact: Option<PathBuf>,
    build_sha256: Option<String>,

    // Step 4: Optional checks
    verify_source: String,
    verify_result: Option<VerifyResult>,
    iso9660_result: Option<Iso9660Compliance>,
    check_field_index: usize,
    check_editing: bool,

    // Shared
    status: String,
    logs: Vec<LogEntry>,
    log_scroll: usize,
    quit_confirm: bool,
}

impl App {
    fn new(doctor: forgeiso_engine::DoctorReport) -> Self {
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

    fn push_log(&mut self, text: String, level: LogLevel) {
        self.logs.push(LogEntry { text, level });
        // Keep scrolled to bottom when new entries arrive.
        let max = self.logs.len().saturating_sub(8);
        if self.log_scroll < max {
            self.log_scroll = max;
        }
    }

    fn invalidate_build_and_checks(&mut self) {
        let artifact = self
            .build_artifact
            .as_ref()
            .map(|path| path.display().to_string());
        if artifact.as_deref() == Some(self.verify_source.as_str()) {
            self.verify_source.clear();
        }

        self.progress.configure_done = false;
        self.progress.build_done = false;
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.build_artifact = None;
        self.build_sha256 = None;
        self.verify_result = None;
        self.iso9660_result = None;
    }

    fn invalidate_checks_only(&mut self) {
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.verify_result = None;
        self.iso9660_result = None;
    }

    fn effective_source(&self) -> String {
        if !self.manual_source.trim().is_empty() {
            return self.manual_source.trim().to_string();
        }
        if let Some(idx) = self.preset_selected {
            let presets = all_presets();
            if let Some(p) = presets.get(idx) {
                if let Some(url) = p.direct_url {
                    return url.to_string();
                }
            }
        }
        String::new()
    }

    fn resolve_distro(&self) -> Option<Distro> {
        match self.distro.trim().to_lowercase().as_str() {
            "fedora" | "rhel" | "rocky" | "alma" | "centos" => Some(Distro::Fedora),
            "mint" => Some(Distro::Mint),
            "arch" => Some(Distro::Arch),
            "ubuntu" | "" => None,
            _ => None,
        }
    }

    fn build_is_complete(&self) -> bool {
        self.progress.build_done
    }

    fn build_inject_config(&self) -> Result<InjectConfig, String> {
        let source_str = self.effective_source();
        if source_str.is_empty() {
            return Err("No ISO source selected".into());
        }

        let opt_str = |s: &str| -> Option<String> {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };

        let split_space = |s: &str| -> Vec<String> {
            s.split_whitespace()
                .filter(|w| !w.is_empty())
                .map(String::from)
                .collect()
        };

        let split_lines = |s: &str| -> Vec<String> {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        };

        let swap = {
            let mb: u32 = self.swap_size_mb.trim().parse().unwrap_or(0);
            if mb > 0 {
                Some(SwapConfig {
                    size_mb: mb,
                    filename: None,
                    swappiness: None,
                })
            } else {
                None
            }
        };

        let grub_timeout: Option<u32> = self.grub_timeout.trim().parse().ok();

        let sysctl: Vec<(String, String)> = self
            .sysctl_pairs
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                let (k, v) = l.split_once('=')?;
                Some((k.trim().to_string(), v.trim().to_string()))
            })
            .collect();

        Ok(InjectConfig {
            source: IsoSource::from_raw(source_str),
            autoinstall_yaml: None,
            out_name: if self.out_name.trim().is_empty() {
                "forgeiso-custom".into()
            } else {
                self.out_name.trim().into()
            },
            output_label: opt_str(&self.output_label),
            expected_sha256: opt_str(&self.expected_sha256),
            hostname: opt_str(&self.hostname),
            username: opt_str(&self.username),
            password: opt_str(&self.password),
            realname: opt_str(&self.realname),
            ssh: SshConfig {
                authorized_keys: split_lines(&self.ssh_keys),
                allow_password_auth: Some(self.ssh_password_auth),
                install_server: Some(self.ssh_install_server),
            },
            network: NetworkConfig {
                dns_servers: split_space(&self.dns_servers),
                ntp_servers: split_space(&self.ntp_servers),
            },
            timezone: opt_str(&self.timezone),
            locale: opt_str(&self.locale),
            keyboard_layout: opt_str(&self.keyboard_layout),
            storage_layout: opt_str(&self.storage_layout),
            apt_mirror: opt_str(&self.apt_mirror),
            extra_packages: split_space(&self.packages),
            wallpaper: None,
            extra_late_commands: split_lines(&self.late_commands),
            no_user_interaction: self.no_user_interaction,
            user: UserConfig {
                groups: split_space(&self.user_groups),
                shell: opt_str(&self.user_shell),
                sudo_nopasswd: self.sudo_nopasswd,
                sudo_commands: Vec::new(),
            },
            firewall: FirewallConfig {
                enabled: self.firewall_enabled,
                default_policy: opt_str(&self.firewall_policy),
                allow_ports: split_space(&self.allow_ports),
                deny_ports: split_space(&self.deny_ports),
            },
            proxy: ProxyConfig {
                http_proxy: opt_str(&self.http_proxy),
                https_proxy: opt_str(&self.https_proxy),
                no_proxy: self
                    .no_proxy
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            },
            static_ip: opt_str(&self.static_ip),
            gateway: opt_str(&self.gateway),
            enable_services: split_space(&self.enable_services),
            disable_services: split_space(&self.disable_services),
            sysctl,
            swap,
            apt_repos: split_lines(&self.apt_repos),
            dnf_repos: split_lines(&self.dnf_repos),
            dnf_mirror: None,
            pacman_repos: Vec::new(),
            pacman_mirror: None,
            containers: ContainerConfig {
                docker: self.docker,
                podman: self.podman,
                docker_users: split_space(&self.docker_users),
            },
            grub: GrubConfig {
                timeout: grub_timeout,
                cmdline_extra: split_space(&self.grub_cmdline),
                default_entry: None,
            },
            encrypt: self.encrypt,
            encrypt_passphrase: opt_str(&self.encrypt_passphrase),
            mounts: split_lines(&self.mounts),
            run_commands: split_lines(&self.run_commands),
            distro: self.resolve_distro(),
        })
    }

    fn validate_step2(&self) -> Option<String> {
        if self.hostname.trim().is_empty() {
            return Some("Hostname is required".into());
        }
        if self.username.trim().is_empty() {
            return Some("Username is required".into());
        }
        if !self.password.is_empty() && self.password != self.password_confirm {
            return Some("Passwords do not match".into());
        }
        if self.output_dir.trim().is_empty() {
            return Some("Output directory is required".into());
        }
        None
    }

    // ── Field accessors for Config tabs ─────────────────────────────────

    fn tab_fields(&self) -> Vec<FieldDef> {
        match self.config_tab {
            ConfigTab::Identity => vec![
                FieldDef::text("Hostname", &self.hostname),
                FieldDef::text("Username", &self.username),
                FieldDef::password("Password", &self.password),
                FieldDef::password("Confirm Password", &self.password_confirm),
                FieldDef::text("Real Name", &self.realname),
                FieldDef::text("Distro", &self.distro),
                FieldDef::text("SSH Keys (1/line)", &self.ssh_keys),
                FieldDef::toggle("SSH Password Auth", self.ssh_password_auth),
                FieldDef::toggle("SSH Install Server", self.ssh_install_server),
            ],
            ConfigTab::Network => vec![
                FieldDef::text("DNS Servers", &self.dns_servers),
                FieldDef::text("NTP Servers", &self.ntp_servers),
                FieldDef::text("Static IP", &self.static_ip),
                FieldDef::text("Gateway", &self.gateway),
                FieldDef::text("HTTP Proxy", &self.http_proxy),
                FieldDef::text("HTTPS Proxy", &self.https_proxy),
                FieldDef::text("No Proxy", &self.no_proxy),
            ],
            ConfigTab::Packages => vec![
                FieldDef::text("Packages", &self.packages),
                FieldDef::text("APT Repos (1/line)", &self.apt_repos),
                FieldDef::text("DNF Repos (1/line)", &self.dnf_repos),
                FieldDef::text("APT Mirror", &self.apt_mirror),
            ],
            ConfigTab::Services => vec![
                FieldDef::text("Enable Services", &self.enable_services),
                FieldDef::text("Disable Services", &self.disable_services),
                FieldDef::toggle("Docker", self.docker),
                FieldDef::toggle("Podman", self.podman),
                FieldDef::text("Docker Users", &self.docker_users),
                FieldDef::toggle("Firewall", self.firewall_enabled),
                FieldDef::text("Firewall Policy", &self.firewall_policy),
                FieldDef::text("Allow Ports", &self.allow_ports),
                FieldDef::text("Deny Ports", &self.deny_ports),
            ],
            ConfigTab::Advanced => vec![
                FieldDef::text("Timezone", &self.timezone),
                FieldDef::text("Locale", &self.locale),
                FieldDef::text("Keyboard Layout", &self.keyboard_layout),
                FieldDef::text("Storage Layout", &self.storage_layout),
                FieldDef::text("User Groups", &self.user_groups),
                FieldDef::text("User Shell", &self.user_shell),
                FieldDef::toggle("Sudo NOPASSWD", self.sudo_nopasswd),
                FieldDef::text("Run Commands (1/line)", &self.run_commands),
                FieldDef::text("Late Commands (1/line)", &self.late_commands),
                FieldDef::text("Sysctl (k=v, 1/line)", &self.sysctl_pairs),
                FieldDef::toggle("Encrypt", self.encrypt),
                FieldDef::password("Encrypt Passphrase", &self.encrypt_passphrase),
                FieldDef::text("Swap Size (MB)", &self.swap_size_mb),
                FieldDef::text("GRUB Timeout", &self.grub_timeout),
                FieldDef::text("GRUB Cmdline", &self.grub_cmdline),
                FieldDef::text("Mounts (1/line)", &self.mounts),
                FieldDef::toggle("No User Interaction", self.no_user_interaction),
            ],
            ConfigTab::Output => vec![
                FieldDef::text("Output Dir", &self.output_dir),
                FieldDef::text("Output Name", &self.out_name),
                FieldDef::text("Output Label", &self.output_label),
                FieldDef::text("Expected SHA-256", &self.expected_sha256),
            ],
        }
    }

    fn tab_field_count(&self) -> usize {
        self.tab_fields().len()
    }

    fn set_field_value(&mut self, idx: usize, value: String) {
        match self.config_tab {
            ConfigTab::Identity => match idx {
                0 => self.hostname = value,
                1 => self.username = value,
                2 => self.password = value,
                3 => self.password_confirm = value,
                4 => self.realname = value,
                5 => self.distro = value,
                6 => self.ssh_keys = value,
                _ => {}
            },
            ConfigTab::Network => match idx {
                0 => self.dns_servers = value,
                1 => self.ntp_servers = value,
                2 => self.static_ip = value,
                3 => self.gateway = value,
                4 => self.http_proxy = value,
                5 => self.https_proxy = value,
                6 => self.no_proxy = value,
                _ => {}
            },
            ConfigTab::Packages => match idx {
                0 => self.packages = value,
                1 => self.apt_repos = value,
                2 => self.dnf_repos = value,
                3 => self.apt_mirror = value,
                _ => {}
            },
            ConfigTab::Services => match idx {
                0 => self.enable_services = value,
                1 => self.disable_services = value,
                4 => self.docker_users = value,
                6 => self.firewall_policy = value,
                7 => self.allow_ports = value,
                8 => self.deny_ports = value,
                _ => {}
            },
            ConfigTab::Advanced => match idx {
                0 => self.timezone = value,
                1 => self.locale = value,
                2 => self.keyboard_layout = value,
                3 => self.storage_layout = value,
                4 => self.user_groups = value,
                5 => self.user_shell = value,
                7 => self.run_commands = value,
                8 => self.late_commands = value,
                9 => self.sysctl_pairs = value,
                11 => self.encrypt_passphrase = value,
                12 => self.swap_size_mb = value,
                13 => self.grub_timeout = value,
                14 => self.grub_cmdline = value,
                15 => self.mounts = value,
                _ => {}
            },
            ConfigTab::Output => match idx {
                0 => self.output_dir = value,
                1 => self.out_name = value,
                2 => self.output_label = value,
                3 => self.expected_sha256 = value,
                _ => {}
            },
        }

        self.invalidate_build_and_checks();
    }

    fn toggle_field(&mut self, idx: usize) {
        match self.config_tab {
            ConfigTab::Identity => match idx {
                7 => self.ssh_password_auth = !self.ssh_password_auth,
                8 => self.ssh_install_server = !self.ssh_install_server,
                _ => {}
            },
            ConfigTab::Services => match idx {
                2 => self.docker = !self.docker,
                3 => self.podman = !self.podman,
                5 => self.firewall_enabled = !self.firewall_enabled,
                _ => {}
            },
            ConfigTab::Advanced => match idx {
                6 => self.sudo_nopasswd = !self.sudo_nopasswd,
                10 => self.encrypt = !self.encrypt,
                16 => self.no_user_interaction = !self.no_user_interaction,
                _ => {}
            },
            _ => {}
        }

        self.invalidate_build_and_checks();
    }

    // ── Build summary ───────────────────────────────────────────────────

    fn summary_lines(&self) -> Vec<(String, String)> {
        let mut lines = Vec::new();
        let src = self.effective_source();
        if !src.is_empty() {
            lines.push(("Source".into(), src));
        }
        let add = |lines: &mut Vec<(String, String)>, label: &str, val: &str| {
            if !val.trim().is_empty() {
                lines.push((label.into(), val.trim().into()));
            }
        };
        add(&mut lines, "Hostname", &self.hostname);
        add(&mut lines, "Username", &self.username);
        if !self.password.is_empty() {
            lines.push(("Password".into(), "(set)".into()));
        }
        add(&mut lines, "Real Name", &self.realname);
        add(&mut lines, "Distro", &self.distro);
        if self.ssh_install_server {
            lines.push(("SSH Server".into(), "yes".into()));
        }
        add(&mut lines, "DNS", &self.dns_servers);
        add(&mut lines, "NTP", &self.ntp_servers);
        add(&mut lines, "Static IP", &self.static_ip);
        add(&mut lines, "Gateway", &self.gateway);
        add(&mut lines, "HTTP Proxy", &self.http_proxy);
        add(&mut lines, "HTTPS Proxy", &self.https_proxy);
        add(&mut lines, "Packages", &self.packages);
        add(&mut lines, "APT Repos", &self.apt_repos);
        add(&mut lines, "DNF Repos", &self.dnf_repos);
        if self.docker {
            lines.push(("Docker".into(), "yes".into()));
        }
        if self.podman {
            lines.push(("Podman".into(), "yes".into()));
        }
        if self.firewall_enabled {
            lines.push(("Firewall".into(), "enabled".into()));
        }
        add(&mut lines, "Timezone", &self.timezone);
        add(&mut lines, "Locale", &self.locale);
        add(&mut lines, "Storage", &self.storage_layout);
        if self.encrypt {
            lines.push(("Encrypt".into(), "yes".into()));
        }
        add(&mut lines, "Swap (MB)", &self.swap_size_mb);
        add(&mut lines, "Output Dir", &self.output_dir);
        add(&mut lines, "Output Name", &self.out_name);
        add(&mut lines, "Output Label", &self.output_label);
        lines
    }

    // ── Spawn operations ────────────────────────────────────────────────

    fn spawn_inject(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        let cfg = match self.build_inject_config() {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("Error: {e}");
                return;
            }
        };
        self.busy = true;
        self.progress.build_done = false;
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.build_artifact = None;
        self.build_sha256 = None;
        self.verify_result = None;
        self.iso9660_result = None;
        self.status = "Building ISO...".into();
        let out_dir = PathBuf::from(&self.output_dir);

        // Subscribe to engine events in the spawned task.
        let mut rx = engine.subscribe();
        let tx2 = tx.clone();
        tokio::spawn(async move {
            loop {
                match rx.try_recv() {
                    Ok(ev) => {
                        let level = match ev.level {
                            forgeiso_engine::EventLevel::Warn => LogLevel::Warn,
                            forgeiso_engine::EventLevel::Error => LogLevel::Error,
                            _ => LogLevel::Info,
                        };
                        let _ = tx2.send(WorkerMsg::EngineEvent(
                            format!("[{:?}] {}", ev.phase, ev.message),
                            level,
                        ));
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(_) => break,
                }
            }
        });

        tokio::spawn(async move {
            let msg = match engine.inject_autoinstall(&cfg, &out_dir).await {
                Ok(r) => WorkerMsg::InjectOk(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("Build failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    fn spawn_verify(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        let source = self.verify_source.trim().to_string();
        if source.is_empty() {
            self.status = "Enter an ISO path to verify".into();
            return;
        }
        self.busy = true;
        self.progress.verify_done = false;
        self.verify_result = None;
        self.status = "Verifying checksum...".into();
        tokio::spawn(async move {
            let msg = match engine.verify(&source, None).await {
                Ok(r) => WorkerMsg::VerifyOk(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("Verify failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    fn spawn_iso9660(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        let source = self.verify_source.trim().to_string();
        if source.is_empty() {
            self.status = "Enter an ISO path to validate".into();
            return;
        }
        self.busy = true;
        self.progress.iso9660_done = false;
        self.iso9660_result = None;
        self.status = "Validating ISO-9660...".into();
        tokio::spawn(async move {
            let msg = match engine.validate_iso9660(&source).await {
                Ok(r) => WorkerMsg::Iso9660Ok(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("ISO-9660 validation failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }
}

// ─── Field definition helper ────────────────────────────────────────────────

enum FieldKind {
    Text,
    Password,
    Toggle(bool),
}

struct FieldDef {
    label: &'static str,
    kind: FieldKind,
    value_str: String,
}

impl FieldDef {
    fn text(label: &'static str, value: &str) -> Self {
        Self {
            label,
            kind: FieldKind::Text,
            value_str: value.to_string(),
        }
    }

    fn password(label: &'static str, value: &str) -> Self {
        Self {
            label,
            kind: FieldKind::Password,
            value_str: value.to_string(),
        }
    }

    fn toggle(label: &'static str, value: bool) -> Self {
        Self {
            label,
            kind: FieldKind::Toggle(value),
            value_str: if value { "ON" } else { "OFF" }.into(),
        }
    }

    fn display_value(&self) -> String {
        match &self.kind {
            FieldKind::Password => {
                if self.value_str.is_empty() {
                    String::new()
                } else {
                    "*".repeat(self.value_str.len())
                }
            }
            _ => self.value_str.clone(),
        }
    }

    fn is_toggle(&self) -> bool {
        matches!(self.kind, FieldKind::Toggle(_))
    }
}

// ─── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let engine = Arc::new(ForgeIsoEngine::new());
    let mut app = App::new(engine.doctor().await);
    let (tx, mut rx_worker) = mpsc::unbounded_channel::<WorkerMsg>();

    loop {
        // Drain worker results.
        while let Ok(msg) = rx_worker.try_recv() {
            match msg {
                WorkerMsg::EngineEvent(text, level) => {
                    app.push_log(text, level);
                }
                WorkerMsg::InspectOk(info) => {
                    app.busy = false;
                    app.detected_distro = info
                        .distro
                        .map(|d| format!("{d:?}"))
                        .or_else(|| Some("Unknown".into()));
                    app.status = "Source inspected".into();
                }
                WorkerMsg::InjectOk(result) => {
                    app.busy = false;
                    app.progress.build_done = true;
                    app.build_artifact = result.artifacts.first().cloned();
                    app.build_sha256 = Some(result.iso.sha256.clone());
                    let label = result
                        .artifacts
                        .first()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| result.output_dir.display().to_string());
                    app.status = format!(
                        "ISO ready: {label} — optional checks are available if you want them"
                    );
                    // Pre-fill verify source with artifact.
                    if let Some(art) = &app.build_artifact {
                        app.verify_source = art.display().to_string();
                    }
                }
                WorkerMsg::VerifyOk(result) => {
                    app.busy = false;
                    app.verify_result = Some(*result);
                    app.progress.verify_done = true;
                    app.status = "Optional checksum check complete".into();
                }
                WorkerMsg::Iso9660Ok(result) => {
                    app.busy = false;
                    app.iso9660_result = Some(*result);
                    app.progress.iso9660_done = true;
                    app.status = "Optional ISO-9660 check complete".into();
                }
                WorkerMsg::OpError(e) => {
                    app.busy = false;
                    app.status = format!("Error: {e}");
                    app.push_log(e, LogLevel::Error);
                }
            }
        }

        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Quit confirmation handling.
                if app.quit_confirm {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => break,
                        _ => {
                            app.quit_confirm = false;
                            app.status = "Ready".into();
                            continue;
                        }
                    }
                }

                // Ctrl-C always quits.
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }

                // Editing mode for text fields.
                if app.editing {
                    match key.code {
                        KeyCode::Esc => {
                            app.editing = false;
                        }
                        KeyCode::Enter => {
                            app.editing = false;
                        }
                        KeyCode::Backspace => {
                            let idx = app.field_index;
                            let mut val = app.get_field_string_raw(idx);
                            val.pop();
                            app.set_field_value(idx, val);
                        }
                        KeyCode::Char(ch) => {
                            let idx = app.field_index;
                            let mut val = app.get_field_string_raw(idx);
                            val.push(ch);
                            app.set_field_value(idx, val);
                        }
                        _ => {}
                    }
                    continue;
                }

                // Optional-checks step editing.
                if app.step == WizardStep::OptionalChecks && app.check_editing {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            app.check_editing = false;
                        }
                        KeyCode::Backspace => {
                            app.verify_source.pop();
                            app.invalidate_checks_only();
                        }
                        KeyCode::Char(ch) => {
                            app.verify_source.push(ch);
                            app.invalidate_checks_only();
                        }
                        _ => {}
                    }
                    continue;
                }

                // Step-specific key handling.
                match app.step {
                    WizardStep::Source => handle_source_keys(&mut app, key.code),
                    WizardStep::Configure => handle_configure_keys(&mut app, key.code),
                    WizardStep::Build => {
                        handle_build_keys(&mut app, key.code, &engine, &tx);
                    }
                    WizardStep::OptionalChecks => {
                        handle_check_keys(&mut app, key.code, &engine, &tx);
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// Helper: get the raw (unmasked) field value for editing.
impl App {
    fn get_field_string_raw(&self, idx: usize) -> String {
        match self.config_tab {
            ConfigTab::Identity => match idx {
                0 => self.hostname.clone(),
                1 => self.username.clone(),
                2 => self.password.clone(),
                3 => self.password_confirm.clone(),
                4 => self.realname.clone(),
                5 => self.distro.clone(),
                6 => self.ssh_keys.clone(),
                _ => String::new(),
            },
            ConfigTab::Network => match idx {
                0 => self.dns_servers.clone(),
                1 => self.ntp_servers.clone(),
                2 => self.static_ip.clone(),
                3 => self.gateway.clone(),
                4 => self.http_proxy.clone(),
                5 => self.https_proxy.clone(),
                6 => self.no_proxy.clone(),
                _ => String::new(),
            },
            ConfigTab::Packages => match idx {
                0 => self.packages.clone(),
                1 => self.apt_repos.clone(),
                2 => self.dnf_repos.clone(),
                3 => self.apt_mirror.clone(),
                _ => String::new(),
            },
            ConfigTab::Services => match idx {
                0 => self.enable_services.clone(),
                1 => self.disable_services.clone(),
                4 => self.docker_users.clone(),
                6 => self.firewall_policy.clone(),
                7 => self.allow_ports.clone(),
                8 => self.deny_ports.clone(),
                _ => String::new(),
            },
            ConfigTab::Advanced => match idx {
                0 => self.timezone.clone(),
                1 => self.locale.clone(),
                2 => self.keyboard_layout.clone(),
                3 => self.storage_layout.clone(),
                4 => self.user_groups.clone(),
                5 => self.user_shell.clone(),
                7 => self.run_commands.clone(),
                8 => self.late_commands.clone(),
                9 => self.sysctl_pairs.clone(),
                11 => self.encrypt_passphrase.clone(),
                12 => self.swap_size_mb.clone(),
                13 => self.grub_timeout.clone(),
                14 => self.grub_cmdline.clone(),
                15 => self.mounts.clone(),
                _ => String::new(),
            },
            ConfigTab::Output => match idx {
                0 => self.output_dir.clone(),
                1 => self.out_name.clone(),
                2 => self.output_label.clone(),
                3 => self.expected_sha256.clone(),
                _ => String::new(),
            },
        }
    }
}

// ─── Key handlers ───────────────────────────────────────────────────────────

fn handle_source_keys(app: &mut App, code: KeyCode) {
    let presets = all_presets();
    match code {
        KeyCode::Char('q') => {
            app.quit_confirm = true;
            app.status = "Quit? Press y to confirm, any other key to cancel".into();
        }
        KeyCode::Tab => {
            app.source_focus = match app.source_focus {
                SourceFocus::PresetList => SourceFocus::ManualInput,
                SourceFocus::ManualInput => SourceFocus::PresetList,
            };
        }
        KeyCode::Up => {
            if app.source_focus == SourceFocus::PresetList && app.preset_scroll > 0 {
                app.preset_scroll -= 1;
                // Auto-select while scrolling.
                app.preset_selected = Some(app.preset_scroll);
                update_detected_distro(app, presets);
                app.progress.source_ready = !app.effective_source().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        KeyCode::Down => {
            if app.source_focus == SourceFocus::PresetList
                && app.preset_scroll < presets.len().saturating_sub(1)
            {
                app.preset_scroll += 1;
                app.preset_selected = Some(app.preset_scroll);
                update_detected_distro(app, presets);
                app.progress.source_ready = !app.effective_source().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        KeyCode::Enter => {
            if app.source_focus == SourceFocus::PresetList {
                app.preset_selected = Some(app.preset_scroll);
                update_detected_distro(app, presets);
                app.manual_source.clear();
                app.progress.source_ready = !app.effective_source().is_empty();
                app.invalidate_build_and_checks();
                app.status = format!(
                    "Selected: {}",
                    presets
                        .get(app.preset_scroll)
                        .map(|p| p.name)
                        .unwrap_or("?")
                );
            } else {
                // Toggle manual input editing — enter simple inline mode.
                app.source_focus = SourceFocus::ManualInput;
                // We handle manual input with direct char entry below.
            }
        }
        KeyCode::Right => {
            if app.effective_source().is_empty() {
                app.status = "Select a preset or enter a path/URL first".into();
            } else {
                app.progress.source_ready = true;
                app.step = WizardStep::Configure;
                app.status = "Source ready — continue with required settings".into();
            }
        }
        KeyCode::Char('n') if app.source_focus == SourceFocus::PresetList => {
            if app.effective_source().is_empty() {
                app.status = "Select a preset or enter a path/URL first".into();
            } else {
                app.progress.source_ready = true;
                app.step = WizardStep::Configure;
                app.status = "Source ready — continue with required settings".into();
            }
        }
        KeyCode::Char(ch) => {
            if app.source_focus == SourceFocus::ManualInput {
                app.manual_source.push(ch);
                app.preset_selected = None;
                app.progress.source_ready = !app.manual_source.trim().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        KeyCode::Backspace => {
            if app.source_focus == SourceFocus::ManualInput {
                app.manual_source.pop();
                app.progress.source_ready = !app.manual_source.trim().is_empty();
                app.invalidate_build_and_checks();
            }
        }
        _ => {}
    }
}

fn update_detected_distro(app: &mut App, presets: &[IsoPreset]) {
    if let Some(idx) = app.preset_selected {
        if let Some(p) = presets.get(idx) {
            app.detected_distro = Some(p.distro.to_string());
            // Auto-set distro field for engine.
            app.distro = p.distro.to_string();
        }
    }
}

fn handle_configure_keys(app: &mut App, code: KeyCode) {
    let field_count = app.tab_field_count();
    match code {
        KeyCode::Char('q') => {
            app.quit_confirm = true;
            app.status = "Quit? Press y to confirm, any other key to cancel".into();
        }
        KeyCode::Tab => {
            app.config_tab = app.config_tab.next();
            app.field_index = 0;
        }
        KeyCode::Up => {
            if app.field_index > 0 {
                app.field_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.field_index + 1 < field_count {
                app.field_index += 1;
            }
        }
        KeyCode::Enter => {
            let fields = app.tab_fields();
            if let Some(f) = fields.get(app.field_index) {
                if f.is_toggle() {
                    app.toggle_field(app.field_index);
                } else {
                    app.editing = true;
                }
            }
        }
        KeyCode::Char(' ') => {
            let fields = app.tab_fields();
            if let Some(f) = fields.get(app.field_index) {
                if f.is_toggle() {
                    app.toggle_field(app.field_index);
                }
            }
        }
        KeyCode::Left | KeyCode::Char('b') => {
            app.step = WizardStep::Source;
            app.status = "Ready".into();
        }
        KeyCode::Right | KeyCode::Char('n') => {
            if let Some(err) = app.validate_step2() {
                app.status = format!("Validation: {err}");
            } else {
                app.progress.configure_done = true;
                app.step = WizardStep::Build;
                app.status = "Ready — review and press Enter to build".into();
            }
        }
        _ => {}
    }
}

fn handle_build_keys(
    app: &mut App,
    code: KeyCode,
    engine: &Arc<ForgeIsoEngine>,
    tx: &mpsc::UnboundedSender<WorkerMsg>,
) {
    match code {
        KeyCode::Char('q') => {
            if app.busy {
                app.quit_confirm = true;
                app.status =
                    "Build is running. Press y to force quit, any other key to cancel".into();
            } else {
                app.quit_confirm = true;
                app.status = "Quit? Press y to confirm, any other key to cancel".into();
            }
        }
        KeyCode::Enter if !app.busy && !app.build_is_complete() => {
            app.spawn_inject(Arc::clone(engine), tx.clone());
        }
        KeyCode::Char('r') if !app.busy && app.build_is_complete() => {
            app.spawn_inject(Arc::clone(engine), tx.clone());
        }
        KeyCode::Char('c') | KeyCode::Char('v') if app.build_is_complete() => {
            app.step = WizardStep::OptionalChecks;
            app.status = "Build complete — optional checks can add extra confidence".into();
        }
        KeyCode::Char('o') if app.build_is_complete() => {
            if let Some(art) = &app.build_artifact {
                if let Some(dir) = art.parent() {
                    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                }
            }
        }
        KeyCode::Left | KeyCode::Char('b') if !app.busy => {
            app.step = WizardStep::Configure;
            app.status = "Ready".into();
        }
        KeyCode::Right | KeyCode::Char('n') if app.build_is_complete() => {
            app.step = WizardStep::OptionalChecks;
            app.status = "Build complete — optional checks are available if you want them".into();
        }
        _ => {}
    }
}

fn handle_check_keys(
    app: &mut App,
    code: KeyCode,
    engine: &Arc<ForgeIsoEngine>,
    tx: &mpsc::UnboundedSender<WorkerMsg>,
) {
    match code {
        KeyCode::Char('q') => {
            app.quit_confirm = true;
            app.status = "Quit? Press y to confirm, any other key to cancel".into();
        }
        KeyCode::Up => {
            if app.check_field_index > 0 {
                app.check_field_index -= 1;
            }
        }
        KeyCode::Down => {
            if app.check_field_index < 2 {
                app.check_field_index += 1;
            }
        }
        KeyCode::Enter => match app.check_field_index {
            0 => {
                app.check_editing = true;
            }
            1 if !app.busy => {
                app.spawn_verify(Arc::clone(engine), tx.clone());
            }
            2 if !app.busy => {
                app.spawn_iso9660(Arc::clone(engine), tx.clone());
            }
            _ => {}
        },
        KeyCode::Left | KeyCode::Char('b') if !app.busy => {
            app.step = WizardStep::Build;
            app.status = "Build complete — you can stop here or run optional checks later".into();
        }
        KeyCode::Backspace if app.check_field_index == 0 => {
            app.verify_source.pop();
            app.invalidate_checks_only();
        }
        KeyCode::Char(ch) if app.check_field_index == 0 => {
            app.verify_source.push(ch);
            app.invalidate_checks_only();
        }
        _ => {}
    }
}

// ─── Rendering ──────────────────────────────────────────────────────────────

fn ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header / step indicator
            Constraint::Min(10),    // main content
            Constraint::Length(3),  // status bar
            Constraint::Length(10), // log panel
            Constraint::Length(3),  // help bar
        ])
        .split(frame.area());

    draw_header(frame, app, outer[0]);
    draw_main(frame, app, outer[1]);
    draw_status(frame, app, outer[2]);
    draw_log_panel(frame, app, outer[3]);
    draw_help_bar(frame, app, outer[4]);
}

fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let titles = WizardStep::ALL
        .iter()
        .map(|step| {
            let style = if *step == app.step {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if app.progress.step_complete(*step) {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::styled(step.label(), style)
        })
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .select(app.step.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL).title(format!(
            " ForgeISO — Step {} of {}: {} ",
            app.step.one_based(),
            WizardStep::ALL.len(),
            app.step.label()
        )));
    frame.render_widget(tabs, area);
}

fn draw_main(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    match app.step {
        WizardStep::Source => draw_source_step(frame, app, area),
        WizardStep::Configure => draw_configure_step(frame, app, area),
        WizardStep::Build => draw_build_step(frame, app, area),
        WizardStep::OptionalChecks => draw_check_step(frame, app, area),
    }
}

fn draw_source_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // preset list
            Constraint::Length(3), // manual input
            Constraint::Length(3), // detected info
        ])
        .split(area);

    // Preset list.
    let presets = all_presets();
    let border_style = if app.source_focus == SourceFocus::PresetList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    // Calculate scroll offset for the list to keep the cursor visible.
    let visible_height = chunks[0].height.saturating_sub(2) as usize;
    let list_offset = if app.preset_scroll >= visible_height {
        app.preset_scroll - visible_height + 1
    } else {
        0
    };

    // Render the list items with manual scroll offset.
    let visible_items: Vec<ListItem<'_>> = presets
        .iter()
        .enumerate()
        .skip(list_offset)
        .take(visible_height)
        .map(|(i, p)| {
            let marker = if app.preset_selected == Some(i) {
                ">"
            } else {
                " "
            };
            let style = if i == app.preset_scroll {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if app.preset_selected == Some(i) {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(Line::styled(
                format!("{marker} {:<30} {:<12} {}", p.name, p.distro, p.note),
                style,
            ))
        })
        .collect();

    let scrolled_list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(
                " Presets ({}/{}) ",
                app.preset_scroll + 1,
                presets.len()
            )),
    );
    frame.render_widget(scrolled_list, chunks[0]);

    // Manual input.
    let input_border = if app.source_focus == SourceFocus::ManualInput {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor = if app.source_focus == SourceFocus::ManualInput {
        "_"
    } else {
        ""
    };
    let input = Paragraph::new(Line::from(format!("{}{}", app.manual_source, cursor))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(input_border)
            .title(" Manual Path/URL (Tab to focus) "),
    );
    frame.render_widget(input, chunks[1]);

    // Detected info.
    let info_text = if let Some(ref d) = app.detected_distro {
        format!("Detected: {d}")
    } else if !app.effective_source().is_empty() {
        "Source set (distro will be auto-detected)".into()
    } else {
        "No source selected".into()
    };
    let info_style = if app.detected_distro.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let info = Paragraph::new(Line::styled(info_text, info_style))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(info, chunks[2]);
}

fn draw_configure_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(5),    // field table
        ])
        .split(area);

    // Tab bar.
    let tab_titles: Vec<Line<'_>> = ConfigTab::ALL
        .iter()
        .map(|t| {
            let style = if *t == app.config_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(t.label(), style)
        })
        .collect();
    let tabs = Tabs::new(tab_titles)
        .select(app.config_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tab to cycle sections "),
        );
    frame.render_widget(tabs, chunks[0]);

    // Fields table.
    let fields = app.tab_fields();
    let rows: Vec<Row<'_>> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let label_style = if i == app.field_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let value_display = f.display_value();
            let value_text = if i == app.field_index && app.editing {
                format!("{value_display}_")
            } else {
                value_display
            };

            let value_style = if f.is_toggle() {
                match f.kind {
                    FieldKind::Toggle(true) => Style::default().fg(Color::Green),
                    FieldKind::Toggle(false) => Style::default().fg(Color::DarkGray),
                    _ => Style::default(),
                }
            } else if i == app.field_index && app.editing {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let indicator = if i == app.field_index { "> " } else { "  " };
            Row::new(vec![
                ratatui::text::Text::styled(format!("{indicator}{}", f.label), label_style),
                ratatui::text::Text::styled(value_text, value_style),
            ])
        })
        .collect();

    let widths = [Constraint::Length(24), Constraint::Min(30)];
    let table = Table::new(rows, widths).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", app.config_tab.label())),
    );
    frame.render_widget(table, chunks[1]);
}

fn draw_build_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    if app.build_is_complete() {
        // Show result.
        let mut lines = vec![
            Line::styled(
                "ISO Ready",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            Line::from("The guided workflow is complete."),
            Line::from("Optional checks are available if you want extra assurance."),
            Line::from(""),
        ];

        if let Some(ref art) = app.build_artifact {
            lines.push(Line::from(format!("Artifact: {}", art.display())));
        }
        if let Some(ref sha) = app.build_sha256 {
            lines.push(Line::from(format!("SHA-256:  {sha}")));
        }

        lines.push(Line::from(""));
        lines.push(Line::styled(
            "  c  Optional checks    o  Open folder    r  Rebuild    q  Quit",
            Style::default().fg(Color::DarkGray),
        ));

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Build Result "),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    } else if app.busy {
        // Show progress.
        let lines = vec![
            Line::styled(
                "Building...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from(""),
            Line::from("Progress updates stream into the log panel below."),
        ];
        let para = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Build "))
            .wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    } else {
        // Show summary card.
        let summary = app.summary_lines();
        let rows: Vec<Row<'_>> = summary
            .iter()
            .map(|(label, value)| {
                Row::new(vec![
                    ratatui::text::Text::styled(
                        label.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    ratatui::text::Text::from(value.clone()),
                ])
            })
            .collect();

        let widths = [Constraint::Length(16), Constraint::Min(30)];
        let table = Table::new(rows, widths).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Review Build Plan — Press Enter to create the ISO "),
        );
        frame.render_widget(table, area);
    }
}

fn draw_check_step(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // intro
            Constraint::Length(3), // verify source input
            Constraint::Length(7), // verify result
            Constraint::Length(9), // iso9660 result
            Constraint::Min(1),    // spacer
        ])
        .split(area);

    let intro = Paragraph::new(vec![
        Line::styled(
            "Optional Checks",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from("Your ISO is already built. Run checksum or ISO-9660 checks only if you want extra confidence."),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Step 4 "))
    .wrap(Wrap { trim: false });
    frame.render_widget(intro, chunks[0]);

    // Source input.
    let input_style = if app.check_field_index == 0 {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor = if app.check_editing { "_" } else { "" };
    let input = Paragraph::new(Line::from(format!("{}{}", app.verify_source, cursor))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(input_style)
            .title(" ISO Path "),
    );
    frame.render_widget(input, chunks[1]);

    // Verify result.
    let verify_lines = if let Some(ref r) = app.verify_result {
        let status_style = if r.matched {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        vec![
            Line::styled(
                if r.matched { "PASS" } else { "FAIL" },
                status_style.add_modifier(Modifier::BOLD),
            ),
            Line::from(format!("File:     {}", r.filename)),
            Line::from(format!("Expected: {}", r.expected)),
            Line::from(format!("Actual:   {}", r.actual)),
        ]
    } else {
        let highlight = if app.check_field_index == 1 {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        vec![Line::styled("Press Enter to verify checksum", highlight)]
    };
    let verify_block = Paragraph::new(verify_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Checksum Verification "),
    );
    frame.render_widget(verify_block, chunks[2]);

    // ISO-9660 result.
    let iso_lines = if let Some(ref r) = app.iso9660_result {
        let status_style = if r.compliant {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };
        vec![
            Line::styled(
                if r.compliant {
                    "ISO-9660 COMPLIANT"
                } else {
                    "NOT COMPLIANT"
                },
                status_style.add_modifier(Modifier::BOLD),
            ),
            Line::from(format!(
                "Volume ID: {}",
                r.volume_id.as_deref().unwrap_or("(none)")
            )),
            Line::from(format!("Size:      {} bytes", r.size_bytes)),
            Line::from(format!(
                "BIOS boot: {}",
                if r.boot_bios { "yes" } else { "no" }
            )),
            Line::from(format!(
                "UEFI boot: {}",
                if r.boot_uefi { "yes" } else { "no" }
            )),
        ]
    } else {
        let highlight = if app.check_field_index == 2 {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        vec![Line::styled("Press Enter to validate ISO-9660", highlight)]
    };
    let iso_block = Paragraph::new(iso_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" ISO-9660 Validation "),
    );
    frame.render_widget(iso_block, chunks[3]);
}

fn draw_status(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let style = if app.status.starts_with("Error") {
        Style::default().fg(Color::Red)
    } else if app.status.starts_with("Validation") || app.busy {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let busy_indicator = if app.busy { " [busy]" } else { "" };
    let para = Paragraph::new(Line::styled(
        format!("{}{}", app.status, busy_indicator),
        style,
    ))
    .block(Block::default().borders(Borders::ALL).title(" Status "));
    frame.render_widget(para, area);
}

fn draw_log_panel(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let total = app.logs.len();
    let start = total.saturating_sub(visible);

    let lines: Vec<Line<'_>> = app
        .logs
        .iter()
        .skip(start)
        .take(visible)
        .map(|entry| {
            let style = match entry.level {
                LogLevel::Info => Style::default().fg(Color::Gray),
                LogLevel::Warn => Style::default().fg(Color::Yellow),
                LogLevel::Error => Style::default().fg(Color::Red),
            };
            Line::styled(entry.text.clone(), style)
        })
        .collect();

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Log ({total} entries) ")),
    );
    frame.render_widget(para, area);
}

fn draw_help_bar(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let help_text = help_text_for_step(app.step, app.busy, app.build_is_complete());

    let para = Paragraph::new(Line::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    ))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(para, area);
}

fn help_text_for_step(step: WizardStep, busy: bool, build_complete: bool) -> &'static str {
    match step {
        WizardStep::Source => {
            "Tab: switch focus | Up/Down: browse | Enter: select | Right/n: next | q: quit"
        }
        WizardStep::Configure => {
            "Tab: sections | Up/Down: fields | Enter: edit | Space: toggle | Left/b: back | Right/n: next | q: quit"
        }
        WizardStep::Build if busy => "Building... please wait | q: quit",
        WizardStep::Build if build_complete => {
            "c: optional checks | o: open folder | r: rebuild | Right/n: optional checks | q: quit"
        }
        WizardStep::Build => "Enter: start build | Left/b: back | q: quit",
        WizardStep::OptionalChecks => {
            "Up/Down: fields | Enter: run action | Left/b: back to build | q: quit"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{help_text_for_step, App, WizardStep};
    use forgeiso_engine::{DoctorReport, GuidedWorkflowProgress, Iso9660Compliance, VerifyResult};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn sample_doctor_report() -> DoctorReport {
        DoctorReport {
            host_os: "linux".into(),
            host_arch: "x86_64".into(),
            linux_supported: true,
            tooling: BTreeMap::new(),
            warnings: Vec::new(),
            timestamp: "2026-03-10T00:00:00Z".into(),
            distro_readiness: BTreeMap::new(),
        }
    }

    #[test]
    fn invalidating_upstream_input_clears_build_and_optional_checks() {
        let mut app = App::new(sample_doctor_report());
        app.progress = GuidedWorkflowProgress {
            source_ready: true,
            configure_done: true,
            build_done: true,
            verify_done: true,
            iso9660_done: true,
        };
        app.build_artifact = Some(PathBuf::from("/tmp/forgeiso.iso"));
        app.build_sha256 = Some("deadbeef".into());
        app.verify_source = "/tmp/forgeiso.iso".into();
        app.verify_result = Some(VerifyResult {
            filename: "forgeiso.iso".into(),
            expected: "abc".into(),
            actual: "def".into(),
            matched: false,
        });
        app.iso9660_result = Some(Iso9660Compliance {
            compliant: true,
            volume_id: Some("FORGEISO".into()),
            size_bytes: 42,
            boot_bios: true,
            boot_uefi: true,
            el_torito_present: true,
            check_method: "iso9660_header".into(),
            error: None,
        });

        app.invalidate_build_and_checks();

        assert!(!app.progress.configure_done);
        assert!(!app.progress.build_done);
        assert!(!app.progress.verify_done);
        assert!(!app.progress.iso9660_done);
        assert!(app.build_artifact.is_none());
        assert!(app.build_sha256.is_none());
        assert!(app.verify_result.is_none());
        assert!(app.iso9660_result.is_none());
        assert!(app.verify_source.is_empty());
    }

    #[test]
    fn build_help_text_marks_optional_checks_as_post_build_work() {
        let help = help_text_for_step(WizardStep::Build, false, true);
        assert!(help.contains("optional checks"));
    }

    #[test]
    fn shared_step_labels_match_guided_product_model() {
        assert_eq!(WizardStep::Source.label(), "Choose ISO");
        assert_eq!(WizardStep::OptionalChecks.label(), "Optional Checks");
    }
}
