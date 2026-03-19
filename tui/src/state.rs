use std::path::PathBuf;

use forgeiso_engine::{
    all_presets, BuildResult, ContainerConfig, Distro, FirewallConfig, GrubConfig,
    GuidedWorkflowProgress, GuidedWorkflowStep, InjectConfig, Iso9660Compliance, IsoMetadata,
    IsoSource, NetworkConfig, ProxyConfig, SshConfig, SwapConfig, UserConfig, VerifyResult,
};

#[allow(dead_code)]
pub(crate) enum WorkerMsg {
    InspectOk(Box<IsoMetadata>),
    InjectOk(Box<BuildResult>),
    EngineEvent(String, LogLevel),
    VerifyOk(Box<VerifyResult>),
    Iso9660Ok(Box<Iso9660Compliance>),
    OpError(String),
}

pub(crate) type WizardStep = GuidedWorkflowStep;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigTab {
    Identity,
    Network,
    Packages,
    Services,
    Advanced,
    Output,
}

impl ConfigTab {
    pub(crate) const ALL: [Self; 6] = [
        Self::Identity,
        Self::Network,
        Self::Packages,
        Self::Services,
        Self::Advanced,
        Self::Output,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Network => "Network",
            Self::Packages => "Packages",
            Self::Services => "Services",
            Self::Advanced => "Advanced",
            Self::Output => "Output",
        }
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    pub(crate) fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceFocus {
    PresetList,
    ManualInput,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
    Info,
    Warn,
    Error,
}

pub(crate) struct LogEntry {
    pub(crate) text: String,
    pub(crate) level: LogLevel,
}

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
    expected_sha256: String,

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

    pub(crate) fn invalidate_build_and_checks(&mut self) {
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

    pub(crate) fn invalidate_checks_only(&mut self) {
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.verify_result = None;
        self.iso9660_result = None;
    }

    pub(crate) fn effective_source(&self) -> String {
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

    pub(crate) fn resolve_distro(&self) -> Option<Distro> {
        match self.distro.trim().to_lowercase().as_str() {
            "fedora" | "rhel" | "rocky" | "alma" | "centos" => Some(Distro::Fedora),
            "mint" => Some(Distro::Mint),
            "arch" => Some(Distro::Arch),
            "ubuntu" | "" => None,
            _ => None,
        }
    }

    pub(crate) fn build_is_complete(&self) -> bool {
        self.progress.build_done
    }

    pub(crate) fn build_inject_config(&self) -> Result<InjectConfig, String> {
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

    pub(crate) fn validate_step2(&self) -> Option<String> {
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

    pub(crate) fn tab_fields(&self) -> Vec<FieldDef> {
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

    pub(crate) fn tab_field_count(&self) -> usize {
        self.tab_fields().len()
    }

    pub(crate) fn set_field_value(&mut self, idx: usize, value: String) {
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

    pub(crate) fn toggle_field(&mut self, idx: usize) {
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

    pub(crate) fn summary_lines(&self) -> Vec<(String, String)> {
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
}

pub(crate) enum FieldKind {
    Text,
    Password,
    Toggle(bool),
}

pub(crate) struct FieldDef {
    pub(crate) label: &'static str,
    pub(crate) kind: FieldKind,
    pub(crate) value_str: String,
}

impl FieldDef {
    pub(crate) fn text(label: &'static str, value: &str) -> Self {
        Self {
            label,
            kind: FieldKind::Text,
            value_str: value.to_string(),
        }
    }

    pub(crate) fn password(label: &'static str, value: &str) -> Self {
        Self {
            label,
            kind: FieldKind::Password,
            value_str: value.to_string(),
        }
    }

    pub(crate) fn toggle(label: &'static str, value: bool) -> Self {
        Self {
            label,
            kind: FieldKind::Toggle(value),
            value_str: if value { "ON" } else { "OFF" }.into(),
        }
    }

    pub(crate) fn display_value(&self) -> String {
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

    pub(crate) fn is_toggle(&self) -> bool {
        matches!(self.kind, FieldKind::Toggle(_))
    }
}

impl App {
    pub(crate) fn get_field_string_raw(&self, idx: usize) -> String {
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
