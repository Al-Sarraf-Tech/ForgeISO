use std::path::PathBuf;

use clap::{Parser, Subcommand};
use forgeiso_engine::sources::format_preset_detail;
use forgeiso_engine::{
    all_presets, emit_launch, find_ovmf, find_preset_by_str, resolve_url, AcquisitionStrategy,
    BuildConfig, ContainerConfig, Distro, EventPhase, FirewallConfig, FirmwareMode, ForgeIsoEngine,
    GrubConfig, Hypervisor, InjectConfig, IsoSource, NetworkConfig, ProfileKind, ProxyConfig,
    SshConfig, SwapConfig, UserConfig, VmLaunchSpec,
};

#[derive(Debug, Parser)]
#[command(name = "forgeiso", version, about = "ForgeISO local bare-metal CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Check host tooling prerequisites (xorriso, qemu, trivy, etc.)
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Read ISO metadata (distro, release, arch, SHA-256) from a local file or URL
    Inspect {
        #[arg(long)]
        source: String,
        #[arg(long)]
        json: bool,
    },
    /// Build a customised ISO from a source image and optional project overlay
    Build {
        #[arg(long, conflicts_with = "preset")]
        source: Option<String>,
        /// Use a built-in source preset instead of --source.
        /// Run 'forgeiso sources list' to see available presets.
        #[arg(long, conflicts_with = "source")]
        preset: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        overlay: Option<PathBuf>,
        #[arg(long)]
        volume_label: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        /// Expected SHA-256 hex digest of the source ISO; operation aborts if it does not match.
        #[arg(long)]
        expected_sha256: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run security scans on a built ISO artifact (trivy, syft, grype, oscap)
    Scan {
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Boot-test an ISO in QEMU (BIOS and/or UEFI) as a smoke test
    Test {
        #[arg(long)]
        iso: PathBuf,
        #[arg(long)]
        bios: bool,
        #[arg(long)]
        uefi: bool,
        #[arg(long)]
        json: bool,
    },
    /// Generate an HTML or JSON report from a build artifact directory
    Report {
        #[arg(long)]
        build: PathBuf,
        #[arg(long)]
        format: String,
    },
    /// Verify ISO integrity against upstream SHA-256 checksums
    Verify {
        #[arg(long)]
        source: String,
        #[arg(long)]
        sums_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Inject an autoinstall/preseed/kickstart configuration into an ISO
    Inject {
        #[arg(long, conflicts_with = "preset")]
        source: Option<String>,
        /// Use a built-in source preset instead of --source.
        /// Run 'forgeiso sources list' to see available presets.
        #[arg(long, conflicts_with = "source")]
        preset: Option<String>,
        #[arg(long)]
        autoinstall: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        volume_label: Option<String>,
        /// Expected SHA-256 hex digest of the source ISO; operation aborts if it does not match.
        #[arg(long)]
        expected_sha256: Option<String>,

        // Identity
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        password_file: Option<PathBuf>,
        #[arg(long)]
        password_stdin: bool,
        #[arg(long)]
        realname: Option<String>,

        // SSH
        #[arg(long, action = clap::ArgAction::Append)]
        ssh_key: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        ssh_key_file: Vec<PathBuf>,
        #[arg(long)]
        ssh_password_auth: bool,
        #[arg(long)]
        no_ssh_password_auth: bool,
        #[arg(long)]
        ssh_install_server: bool,
        #[arg(long)]
        no_ssh_install_server: bool,

        // Network
        #[arg(long, action = clap::ArgAction::Append)]
        dns: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        ntp_server: Vec<String>,

        // System
        #[arg(long)]
        timezone: Option<String>,
        #[arg(long)]
        locale: Option<String>,
        #[arg(long)]
        keyboard_layout: Option<String>,

        // Storage/Apt
        #[arg(long)]
        storage_layout: Option<String>,
        #[arg(long)]
        apt_mirror: Option<String>,

        // Packages
        #[arg(long, action = clap::ArgAction::Append)]
        package: Vec<String>,

        // Branding
        #[arg(long)]
        wallpaper: Option<PathBuf>,

        // Escape hatches
        #[arg(long, action = clap::ArgAction::Append)]
        late_command: Vec<String>,
        #[arg(long)]
        no_user_interaction: bool,

        // User & access management
        #[arg(long, action = clap::ArgAction::Append)]
        group: Vec<String>,
        #[arg(long)]
        shell: Option<String>,
        #[arg(long)]
        sudo_nopasswd: bool,
        #[arg(long, action = clap::ArgAction::Append)]
        sudo_command: Vec<String>,

        // Firewall
        #[arg(long)]
        firewall: bool,
        #[arg(long)]
        firewall_policy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        allow_port: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        deny_port: Vec<String>,

        // Network extras
        #[arg(long)]
        static_ip: Option<String>,
        #[arg(long)]
        gateway: Option<String>,
        #[arg(long)]
        http_proxy: Option<String>,
        #[arg(long)]
        https_proxy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        no_proxy: Vec<String>,

        // Services
        #[arg(long, action = clap::ArgAction::Append)]
        enable_service: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append)]
        disable_service: Vec<String>,

        // Kernel
        #[arg(long, action = clap::ArgAction::Append)]
        sysctl: Vec<String>,

        // Swap
        #[arg(long)]
        swap_size: Option<u32>,
        #[arg(long)]
        swap_file: Option<String>,
        #[arg(long)]
        swappiness: Option<u8>,

        // APT repos (Ubuntu/Debian)
        #[arg(long, action = clap::ArgAction::Append)]
        apt_repo: Vec<String>,

        // DNF repos (Fedora/RHEL) — full "[id]\nbaseurl=..." stanza or URL
        #[arg(long, action = clap::ArgAction::Append)]
        dnf_repo: Vec<String>,
        /// Override the primary Fedora/RHEL DNF mirror base URL
        #[arg(long)]
        dnf_mirror: Option<String>,

        // Pacman repos (Arch) — "Server = https://..." mirror lines
        #[arg(long, action = clap::ArgAction::Append)]
        pacman_repo: Vec<String>,
        /// Override the primary Arch Linux pacman mirror URL
        #[arg(long)]
        pacman_mirror: Option<String>,

        // Containers
        #[arg(long)]
        docker: bool,
        #[arg(long)]
        podman: bool,
        #[arg(long, action = clap::ArgAction::Append)]
        docker_user: Vec<String>,

        // GRUB
        #[arg(long)]
        grub_timeout: Option<u32>,
        #[arg(long, action = clap::ArgAction::Append)]
        grub_cmdline: Vec<String>,
        #[arg(long)]
        grub_default: Option<String>,

        // Encryption
        #[arg(long)]
        encrypt: bool,
        #[arg(long)]
        encrypt_passphrase: Option<String>,
        #[arg(long)]
        encrypt_passphrase_file: Option<PathBuf>,

        // Mounts
        #[arg(long, action = clap::ArgAction::Append)]
        mount: Vec<String>,

        // Run commands
        #[arg(long, action = clap::ArgAction::Append)]
        run_command: Vec<String>,

        // Target distro: ubuntu (default), fedora, arch
        #[arg(long, value_name = "DISTRO")]
        distro: Option<String>,

        #[arg(long)]
        json: bool,
    },
    /// Show which files differ between two ISO images
    Diff {
        #[arg(long)]
        base: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Work with built-in ISO source presets
    Sources {
        #[command(subcommand)]
        command: SourcesCmd,
    },
    /// Generate VM hypervisor launch commands for a local ISO
    Vm {
        #[command(subcommand)]
        command: VmCmd,
    },
}

#[derive(Debug, Subcommand)]
enum SourcesCmd {
    /// List all built-in ISO source presets
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show details for a specific preset
    Show {
        /// Preset name, e.g. ubuntu-server-lts
        preset: String,
        #[arg(long)]
        json: bool,
    },
    /// Resolve the download URL for a preset
    Resolve {
        /// Preset name, e.g. ubuntu-server-lts
        preset: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum VmCmd {
    /// Emit hypervisor launch commands for testing an ISO in a VM
    Emit {
        /// Path to the ISO to boot
        #[arg(long, value_name = "PATH")]
        iso: PathBuf,
        /// Hypervisor: qemu (default), virtualbox, vmware, hyperv, proxmox
        #[arg(long, default_value = "qemu")]
        hypervisor: String,
        /// Firmware: bios (default) or uefi
        #[arg(long, default_value = "bios")]
        firmware: String,
        /// RAM in MiB (default: 2048)
        #[arg(long, default_value_t = 2048)]
        ram: u32,
        /// vCPUs (default: 2)
        #[arg(long, default_value_t = 2)]
        cpus: u8,
        /// Disk size in GiB for QEMU disk image creation (default: 20)
        #[arg(long, default_value_t = 20)]
        disk: u32,
        /// VM name (defaults to ISO stem)
        #[arg(long)]
        name: Option<String>,
        /// Emit output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let engine = ForgeIsoEngine::new();

    // Subscribe to engine events and spawn event handler
    let mut rx = engine.subscribe();
    let _event_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event.phase {
                EventPhase::Download => {
                    eprint!("\r[Download] {:<40}", event.message);
                    let _ = std::io::Write::flush(&mut std::io::stderr());
                }
                _ => {
                    eprintln!("[{:?}] {}", event.phase, event.message);
                }
            }
        }
    });

    match cli.command {
        Commands::Doctor { json } => {
            let report = engine.doctor().await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("ForgeISO doctor @ {}", report.timestamp);
                println!("Host: {} {}", report.host_os, report.host_arch);
                println!("Linux build support: {}", report.linux_supported);
                println!("Tooling:");
                for (name, available) in &report.tooling {
                    let marker = if *available { "ok" } else { "MISSING" };
                    println!("  [{marker}] {name}");
                }
                println!("Distro readiness:");
                for (distro, ready) in &report.distro_readiness {
                    let marker = if *ready { "ready" } else { "not ready" };
                    println!("  [{marker}] {distro}");
                }
                for warning in &report.warnings {
                    println!("warning: {warning}");
                }
                println!("Source presets:");
                println!("  {} built-in presets available", all_presets().len());
                println!("  Run 'forgeiso sources list' to see all");
            }
        }
        Commands::Inspect { source, json } => {
            let info = engine.inspect_source(&source, None).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                println!("Source: {}", info.source_value);
                println!("Cached path: {}", info.source_path.display());
                println!(
                    "Detected: distro={} release={} arch={}",
                    info.distro
                        .map(|value| format!("{:?}", value))
                        .unwrap_or_else(|| "unknown".to_string()),
                    info.release.as_deref().unwrap_or("unknown"),
                    info.architecture.as_deref().unwrap_or("unknown")
                );
                println!(
                    "Volume ID: {}",
                    info.volume_id.as_deref().unwrap_or("unknown")
                );
                if !info.warnings.is_empty() {
                    println!("Warnings:");
                    for warning in info.warnings {
                        println!("  - {warning}");
                    }
                }
            }
        }
        Commands::Build {
            source,
            preset,
            project,
            out,
            name,
            overlay,
            volume_label,
            profile,
            expected_sha256,
            json,
        } => {
            let cfg = if let Some(project) = project {
                BuildConfig::from_path(&project)?
            } else {
                // Resolve source: --preset takes precedence over --source when both absent
                let resolved_source = resolve_source_from_preset_or_str(source, preset)?;
                BuildConfig {
                    name: name.unwrap_or_else(|| "forgeiso-build".to_string()),
                    source: IsoSource::from_raw(resolved_source),
                    overlay_dir: overlay,
                    output_label: volume_label,
                    profile: parse_profile(profile.as_deref().unwrap_or("minimal"))?,
                    auto_scan: false,
                    auto_test: false,
                    scanning: Default::default(),
                    testing: Default::default(),
                    keep_workdir: false,
                    expected_sha256,
                }
            };

            let result = engine.build(&cfg, &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                if let Some(iso) = result.artifacts.first() {
                    println!("Built ISO: {}", iso.display());
                }
                println!("Report JSON: {}", result.report_json.display());
                println!("Report HTML: {}", result.report_html.display());
                println!(
                    "Detected source: distro={} release={}",
                    result
                        .iso
                        .distro
                        .map(|value| format!("{:?}", value))
                        .unwrap_or_else(|| "unknown".to_string()),
                    result.iso.release.as_deref().unwrap_or("unknown")
                );
            }
        }
        Commands::Scan {
            artifact,
            policy,
            json,
        } => {
            let out = artifact
                .parent()
                .map(|p| p.join("scan"))
                .unwrap_or_else(|| PathBuf::from("scan"));
            let result = engine.scan(&artifact, policy.as_deref(), &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("scan report: {}", result.report_json.display());
                for report in result.report.reports {
                    println!(
                        "  - {}: {:?} ({})",
                        report.tool, report.status, report.message
                    );
                }
            }
        }
        Commands::Test {
            iso,
            bios,
            uefi,
            json,
        } => {
            let run_bios = bios || !uefi;
            let run_uefi = uefi || !bios;
            let out = iso
                .parent()
                .map(|p| p.join("test"))
                .unwrap_or_else(|| PathBuf::from("test"));
            let result = engine.test_iso(&iso, run_bios, run_uefi, &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "bios={} uefi={} passed={}",
                    result.bios, result.uefi, result.passed
                );
                for log in result.logs {
                    println!("  - {}", log.display());
                }
            }
        }
        Commands::Report { build, format } => {
            let path = engine.report(&build, &format).await?;
            println!("{}", path.display());
        }
        Commands::Verify {
            source,
            sums_url,
            json,
        } => {
            let result = engine.verify(&source, sums_url.as_deref()).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Verifying: {}", result.filename);
                println!("Expected: {}", result.expected);
                println!("Actual:   {}", result.actual);
                println!("Match:    {}", result.matched);
            }
        }
        Commands::Inject {
            source,
            preset,
            autoinstall,
            out,
            name,
            volume_label,
            hostname,
            username,
            password,
            password_file,
            password_stdin,
            realname,
            ssh_key,
            ssh_key_file,
            ssh_password_auth,
            no_ssh_password_auth,
            ssh_install_server,
            no_ssh_install_server,
            dns,
            ntp_server,
            timezone,
            locale,
            keyboard_layout,
            storage_layout,
            apt_mirror,
            package,
            wallpaper,
            late_command,
            no_user_interaction,
            group,
            shell,
            sudo_nopasswd,
            sudo_command,
            firewall,
            firewall_policy,
            allow_port,
            deny_port,
            static_ip,
            gateway,
            http_proxy,
            https_proxy,
            no_proxy,
            enable_service,
            disable_service,
            sysctl,
            swap_size,
            swap_file,
            swappiness,
            apt_repo,
            dnf_repo,
            dnf_mirror,
            pacman_repo,
            pacman_mirror,
            docker,
            podman,
            docker_user,
            grub_timeout,
            grub_cmdline,
            grub_default,
            encrypt,
            encrypt_passphrase,
            encrypt_passphrase_file,
            mount,
            run_command,
            distro,
            expected_sha256,
            json,
        } => {
            // Resolve password (priority: stdin > file > cli arg)
            let resolved_password = if password_stdin {
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf)?;
                Some(buf.trim().to_string())
            } else if let Some(ref pf) = password_file {
                Some(std::fs::read_to_string(pf)?.trim().to_string())
            } else {
                password
            };

            // Read SSH keys from files
            let mut all_ssh_keys = ssh_key;
            for kf in ssh_key_file {
                all_ssh_keys.push(std::fs::read_to_string(&kf)?.trim().to_string());
            }

            // Resolve encryption passphrase
            let resolved_encrypt_passphrase = if let Some(ref f) = encrypt_passphrase_file {
                Some(std::fs::read_to_string(f)?.trim().to_string())
            } else {
                encrypt_passphrase
            };

            // Parse distro
            let resolved_distro = match distro.as_deref() {
                None | Some("ubuntu") => None,
                Some("fedora") => Some(Distro::Fedora),
                Some("arch") => Some(Distro::Arch),
                Some("mint") => Some(Distro::Mint),
                Some(other) => {
                    eprintln!("ERROR: unknown distro '{other}'. Valid: ubuntu, fedora, arch, mint");
                    std::process::exit(1);
                }
            };

            // Parse sysctl "key=value" pairs — warn on malformed entries
            let sysctl_pairs: Vec<(String, String)> = sysctl
                .iter()
                .filter_map(|s| {
                    let mut parts = s.splitn(2, '=');
                    match (parts.next(), parts.next()) {
                        (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
                        _ => {
                            eprintln!(
                                "WARNING: --sysctl {s:?} ignored (expected key=value format)"
                            );
                            None
                        }
                    }
                })
                .collect();

            // Resolve source: --preset or --source
            let resolved_source = resolve_source_from_preset_or_str(source, preset)?;

            let cfg = InjectConfig {
                source: IsoSource::from_raw(resolved_source),
                autoinstall_yaml: autoinstall,
                out_name: name.unwrap_or_else(|| "injected.iso".to_string()),
                output_label: volume_label,
                hostname,
                username,
                password: resolved_password,
                realname,
                ssh: SshConfig {
                    authorized_keys: all_ssh_keys,
                    allow_password_auth: if ssh_password_auth {
                        Some(true)
                    } else if no_ssh_password_auth {
                        Some(false)
                    } else {
                        None
                    },
                    install_server: if ssh_install_server {
                        Some(true)
                    } else if no_ssh_install_server {
                        Some(false)
                    } else {
                        None
                    },
                },
                network: NetworkConfig {
                    dns_servers: dns,
                    ntp_servers: ntp_server,
                },
                timezone,
                locale,
                keyboard_layout,
                storage_layout,
                apt_mirror,
                extra_packages: package,
                wallpaper,
                extra_late_commands: late_command,
                no_user_interaction,
                user: UserConfig {
                    groups: group,
                    shell,
                    sudo_nopasswd,
                    sudo_commands: sudo_command,
                },
                firewall: FirewallConfig {
                    enabled: firewall,
                    default_policy: firewall_policy,
                    allow_ports: allow_port,
                    deny_ports: deny_port,
                },
                proxy: ProxyConfig {
                    http_proxy,
                    https_proxy,
                    no_proxy,
                },
                static_ip,
                gateway,
                enable_services: enable_service,
                disable_services: disable_service,
                sysctl: sysctl_pairs,
                swap: {
                    if swap_size.is_none() {
                        if swap_file.is_some() {
                            eprintln!("WARNING: --swap-file ignored without --swap-size");
                        }
                        if swappiness.is_some() {
                            eprintln!("WARNING: --swappiness ignored without --swap-size");
                        }
                    }
                    swap_size.map(|mb| SwapConfig {
                        size_mb: mb,
                        filename: swap_file,
                        swappiness,
                    })
                },
                apt_repos: apt_repo,
                dnf_repos: dnf_repo,
                dnf_mirror,
                pacman_repos: pacman_repo,
                pacman_mirror,
                containers: ContainerConfig {
                    docker,
                    podman,
                    docker_users: docker_user,
                },
                grub: GrubConfig {
                    timeout: grub_timeout,
                    cmdline_extra: grub_cmdline,
                    default_entry: grub_default,
                },
                encrypt,
                encrypt_passphrase: resolved_encrypt_passphrase,
                mounts: mount,
                run_commands: run_command,
                distro: resolved_distro,
                expected_sha256,
            };
            let result = engine.inject_autoinstall(&cfg, &out).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else if let Some(iso) = result.artifacts.first() {
                println!("Injected ISO: {}", iso.display());
            }
        }
        Commands::Sources { command } => match command {
            SourcesCmd::List { json } => {
                let presets = all_presets();
                if json {
                    println!("{}", serde_json::to_string_pretty(presets)?);
                } else {
                    println!("{:<28} {:<12} {:<16} NOTE", "PRESET", "DISTRO", "STRATEGY");
                    println!("{}", "-".repeat(90));
                    for p in presets {
                        println!(
                            "{:<28} {:<12} {:<16} {}",
                            p.id.as_str(),
                            p.distro,
                            p.strategy.as_str(),
                            &p.note.chars().take(50).collect::<String>()
                        );
                    }
                    println!(
                        "\n{} presets. Run 'forgeiso sources show <PRESET>' for details.",
                        presets.len()
                    );
                }
            }
            SourcesCmd::Show { preset, json } => {
                let p = find_preset_by_str(&preset).ok_or_else(|| {
                    let ids: Vec<_> = all_presets().iter().map(|p| p.id.as_str()).collect();
                    anyhow::anyhow!("unknown preset '{}'. Available: {}", preset, ids.join(", "))
                })?;
                if json {
                    println!("{}", serde_json::to_string_pretty(p)?);
                } else {
                    println!("{}", format_preset_detail(p));
                }
            }
            SourcesCmd::Resolve { preset, json } => {
                let p = find_preset_by_str(&preset).ok_or_else(|| {
                    let ids: Vec<_> = all_presets().iter().map(|p| p.id.as_str()).collect();
                    anyhow::anyhow!("unknown preset '{}'. Available: {}", preset, ids.join(", "))
                })?;
                let url = resolve_url(p)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "preset": p.id.as_str(),
                            "strategy": p.strategy.as_str(),
                            "url": url,
                            "official_page": p.official_page,
                            "checksum_url": p.checksum_url,
                            "note": p.note,
                        }))?
                    );
                } else {
                    match &url {
                        Some(u) => {
                            println!("Preset:        {}", p.id.as_str());
                            println!("URL:           {u}");
                            if let Some(c) = p.checksum_url {
                                println!("Checksums:     {c}");
                            }
                        }
                        None => {
                            match p.strategy {
                                AcquisitionStrategy::DiscoveryPage => {
                                    println!("Preset:        {}", p.id.as_str());
                                    println!(
                                        "Strategy:      discovery-page (URL changes each release)"
                                    );
                                    println!("Official page: {}", p.official_page);
                                    println!("Note:          {}", p.note);
                                    println!("\nVisit the official page to find the current download URL,");
                                    println!("then use: forgeiso inject --source <URL> ...");
                                }
                                AcquisitionStrategy::UserProvided => {
                                    println!("Preset:        {}", p.id.as_str());
                                    println!("Strategy:      user-provided (BYO ISO)");
                                    println!("Official page: {}", p.official_page);
                                    println!("Note:          {}", p.note);
                                    println!("\nProvide your own ISO path: forgeiso inject --source /path/to/rhel.iso ...");
                                }
                                AcquisitionStrategy::DirectUrl => {
                                    eprintln!(
                                        "error: preset '{}' is DirectUrl but has no URL configured",
                                        p.id.as_str()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        },
        Commands::Diff { base, target, json } => {
            let result = engine.diff_isos(&base, &target).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("ISO Diff: {} vs {}", base.display(), target.display());
                println!();
                if !result.added.is_empty() {
                    println!("Added ({}):", result.added.len());
                    for file in &result.added {
                        println!("  + {}", file);
                    }
                    println!();
                }
                if !result.removed.is_empty() {
                    println!("Removed ({}):", result.removed.len());
                    for file in &result.removed {
                        println!("  - {}", file);
                    }
                    println!();
                }
                if !result.modified.is_empty() {
                    println!("Modified ({}):", result.modified.len());
                    for entry in &result.modified {
                        println!(
                            "  ~ {} ({} → {})",
                            entry.path, entry.base_size, entry.target_size
                        );
                    }
                    println!();
                }
                println!("Unchanged: {}", result.unchanged);
            }
        }
        Commands::Vm { command } => match command {
            VmCmd::Emit {
                iso,
                hypervisor,
                firmware,
                ram,
                cpus,
                disk,
                name,
                json,
            } => {
                let hv = match hypervisor.to_lowercase().as_str() {
                    "qemu" => Hypervisor::Qemu,
                    "virtualbox" | "vbox" => Hypervisor::VirtualBox,
                    "vmware" => Hypervisor::Vmware,
                    "hyperv" | "hyper-v" => Hypervisor::HyperV,
                    "proxmox" => Hypervisor::Proxmox,
                    other => anyhow::bail!("unknown hypervisor '{other}': expected qemu, virtualbox, vmware, hyperv, proxmox"),
                };
                let fw = match firmware.to_lowercase().as_str() {
                    "bios" => FirmwareMode::Bios,
                    "uefi" | "efi" => FirmwareMode::Uefi,
                    other => anyhow::bail!("unknown firmware '{other}': expected bios or uefi"),
                };
                let ovmf = if matches!(fw, FirmwareMode::Uefi) {
                    find_ovmf()
                } else {
                    None
                };
                let vm_name = name.unwrap_or_else(|| {
                    iso.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "forgeiso-vm".to_string())
                });
                let spec = VmLaunchSpec {
                    hypervisor: hv,
                    firmware: fw,
                    iso_path: iso,
                    ram_mb: ram,
                    cpus,
                    disk_gb: disk,
                    vm_name,
                    ovmf_path: ovmf,
                };
                let out = emit_launch(&spec);
                if json {
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    println!("Hypervisor: {:?}", out.hypervisor);
                    println!("Firmware:   {:?}", out.firmware);
                    println!("ISO:        {}", out.iso_path);
                    println!(
                        "KVM:        {}",
                        if out.kvm_available {
                            "available"
                        } else {
                            "not available (software emulation)"
                        }
                    );
                    if let Some(ref ovmf) = out.ovmf_used {
                        println!("OVMF:       {ovmf}");
                    }
                    if !out.notes.is_empty() {
                        println!();
                        for note in &out.notes {
                            eprintln!("NOTE: {note}");
                        }
                    }
                    if !out.commands.is_empty() {
                        println!();
                        // QEMU args are a single argv list; join as one shell command.
                        // VirtualBox/Proxmox store complete commands, one per entry.
                        if matches!(out.hypervisor, Hypervisor::Qemu) {
                            println!("# Run:");
                            println!("{}", out.commands.join(" \\\n  "));
                        } else {
                            println!("# Run these commands:");
                            for cmd in &out.commands {
                                println!("{cmd}");
                            }
                        }
                    }
                    if let Some(ref script) = out.script {
                        println!();
                        println!("# Script:");
                        println!("{script}");
                    }
                }
            }
        },
    }

    Ok(())
}

fn parse_profile(raw: &str) -> anyhow::Result<ProfileKind> {
    match raw {
        "minimal" => Ok(ProfileKind::Minimal),
        "desktop" => Ok(ProfileKind::Desktop),
        other => anyhow::bail!("unsupported profile '{other}': expected minimal|desktop"),
    }
}

/// Resolve a source URL/path from either --preset or --source flags.
/// Returns an error if neither is provided or if the preset strategy requires user input.
fn resolve_source_from_preset_or_str(
    source: Option<String>,
    preset: Option<String>,
) -> anyhow::Result<String> {
    if let Some(preset_name) = preset {
        let ids: Vec<&str> = all_presets().iter().map(|p| p.id.as_str()).collect();
        let found = find_preset_by_str(&preset_name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown preset '{}'. Available: {}",
                preset_name,
                ids.join(", ")
            )
        })?;
        match found.strategy {
            AcquisitionStrategy::DirectUrl => {
                let url = resolve_url(found)?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "preset '{}' is DirectUrl but has no direct_url configured",
                        found.id.as_str()
                    )
                })?;
                Ok(url)
            }
            AcquisitionStrategy::DiscoveryPage => {
                anyhow::bail!(
                    "preset '{}' uses a discovery page — visit {} to find the current ISO URL, \
                     then use --source <URL>",
                    found.id.as_str(),
                    found.official_page
                );
            }
            AcquisitionStrategy::UserProvided => {
                anyhow::bail!(
                    "preset '{}' requires you to supply your own ISO — visit {} and use --source <path>",
                    found.id.as_str(),
                    found.official_page
                );
            }
        }
    } else if let Some(s) = source {
        Ok(s)
    } else {
        anyhow::bail!("--source or --preset is required")
    }
}
