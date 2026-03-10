use std::path::PathBuf;

use clap::{Parser, Subcommand};
use forgeiso_engine::sources::format_preset_detail;
use forgeiso_engine::{
    all_presets, emit_launch, find_ovmf, find_preset_by_str, resolve_url, AcquisitionStrategy,
    BuildConfig, ContainerConfig, Distro, EventPhase, FirewallConfig, FirmwareMode, ForgeIsoEngine,
    GrubConfig, Hypervisor, InjectConfig, IsoSource, NetworkConfig, ProfileKind, ProxyConfig,
    SshConfig, SwapConfig, UserConfig, VmLaunchSpec,
};

const CLI_AFTER_HELP: &str = "\
GUIDED INTERFACES:
    forgeiso-desktop  Desktop wizard for normal users
    forgeiso-tui      Guided terminal workflow for operators

ADVANCED CLI:
    forgeiso is the explicit interface for scripting, CI, and power-user workflows.
    Use `forgeiso inject`, `build`, `scan`, `test`, and `report` directly when you
    want stable flags, machine-readable output, or full control over the pipeline.
";

#[derive(Debug, Parser)]
#[command(
    name = "forgeiso",
    version,
    about = "Advanced CLI for local Linux ISO automation",
    arg_required_else_help = true,
    after_help = CLI_AFTER_HELP
)]
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
    /// Build a customised ISO artifact from a source image and optional overlay
    Build {
        #[arg(long, conflicts_with = "preset", help_heading = "Source")]
        source: Option<String>,
        /// Use a built-in source preset instead of --source.
        /// Run 'forgeiso sources list' to see available presets.
        #[arg(long, conflicts_with = "source", help_heading = "Source")]
        preset: Option<String>,
        /// Load a build definition from a project file instead of individual flags.
        #[arg(long, help_heading = "Source")]
        project: Option<PathBuf>,
        /// Directory to write the output ISO into
        #[arg(long, help_heading = "Output")]
        out: PathBuf,
        /// Output ISO filename (e.g., my-server.iso)
        #[arg(long, help_heading = "Output")]
        name: Option<String>,
        /// Directory of files to overlay into the ISO root filesystem.
        #[arg(long, help_heading = "Output")]
        overlay: Option<PathBuf>,
        #[arg(long, help_heading = "Output")]
        volume_label: Option<String>,
        /// Build profile: minimal or desktop.
        #[arg(
            long,
            help_heading = "Output",
            value_name = "PROFILE",
            value_parser = ["minimal", "desktop"]
        )]
        profile: Option<String>,
        /// Expected SHA-256 hex digest of the source ISO; operation aborts if it does not match.
        #[arg(long, help_heading = "Output")]
        expected_sha256: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run security scans on a built ISO artifact (trivy, syft, grype, oscap)
    Scan {
        /// Built ISO artifact to scan.
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
        /// Report format: html or json.
        #[arg(long, value_name = "FORMAT", value_parser = ["html", "json"])]
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
    /// Inject a distro-appropriate unattended install configuration into an ISO
    #[command(after_long_help = "\
EXAMPLES:
    # Minimal Ubuntu server ISO
    forgeiso inject --source ubuntu-24.04.iso --out /tmp --hostname myserver --username admin --password secret

    # Fedora with Docker and firewall
    forgeiso inject --preset fedora-server --out /tmp --hostname web01 --username ops --password secret --docker --firewall --allow-port 22 --allow-port 443

    # From preset with SSH keys
    forgeiso inject --preset ubuntu-server-lts --out /tmp --hostname prod01 --username deploy --ssh-key \"ssh-ed25519 AAAA... user@host\" --sudo-nopasswd
")]
    Inject {
        #[arg(long, conflicts_with = "preset", help_heading = "Source")]
        source: Option<String>,
        /// Use a built-in source preset instead of --source.
        /// Run 'forgeiso sources list' to see available presets.
        #[arg(long, conflicts_with = "source", help_heading = "Source")]
        preset: Option<String>,
        /// Merge CLI flags into an existing Ubuntu autoinstall YAML.
        #[arg(long, help_heading = "Source")]
        autoinstall: Option<PathBuf>,
        /// Installer path override: ubuntu, fedora, mint, or arch.
        #[arg(
            long,
            value_name = "DISTRO",
            help_heading = "Source",
            value_parser = ["ubuntu", "fedora", "mint", "arch"]
        )]
        distro: Option<String>,
        #[arg(long, help_heading = "Source")]
        json: bool,

        // Identity
        #[arg(long, help_heading = "Identity")]
        hostname: Option<String>,
        #[arg(long, help_heading = "Identity")]
        username: Option<String>,
        #[arg(long, help_heading = "Identity")]
        password: Option<String>,
        #[arg(long, help_heading = "Identity")]
        password_file: Option<PathBuf>,
        #[arg(long, help_heading = "Identity")]
        password_stdin: bool,
        #[arg(long, help_heading = "Identity")]
        realname: Option<String>,

        // SSH
        #[arg(long, action = clap::ArgAction::Append, help_heading = "SSH")]
        ssh_key: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "SSH")]
        ssh_key_file: Vec<PathBuf>,
        #[arg(long, help_heading = "SSH")]
        ssh_password_auth: bool,
        #[arg(long, help_heading = "SSH")]
        no_ssh_password_auth: bool,
        #[arg(long, help_heading = "SSH")]
        ssh_install_server: bool,
        #[arg(long, help_heading = "SSH")]
        no_ssh_install_server: bool,

        // Network
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Network")]
        dns: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Network")]
        ntp_server: Vec<String>,
        #[arg(long, help_heading = "Network")]
        static_ip: Option<String>,
        #[arg(long, help_heading = "Network")]
        gateway: Option<String>,
        #[arg(long, help_heading = "Network")]
        http_proxy: Option<String>,
        #[arg(long, help_heading = "Network")]
        https_proxy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Network")]
        no_proxy: Vec<String>,

        // System
        #[arg(long, help_heading = "System")]
        timezone: Option<String>,
        #[arg(long, help_heading = "System")]
        locale: Option<String>,
        #[arg(long, help_heading = "System")]
        keyboard_layout: Option<String>,
        #[arg(long, help_heading = "System")]
        storage_layout: Option<String>,
        /// Override the Ubuntu/Debian package mirror used during install.
        #[arg(long, help_heading = "System")]
        apt_mirror: Option<String>,

        // Packages & Repos
        /// Extra package to install. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        package: Vec<String>,
        /// Additional APT repository entry. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        apt_repo: Vec<String>,
        /// Additional DNF repository stanza or URL. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        dnf_repo: Vec<String>,
        /// Override the primary Fedora/RHEL DNF mirror base URL
        #[arg(long, help_heading = "Packages & Repos")]
        dnf_mirror: Option<String>,
        /// Additional pacman repository mirror line. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        pacman_repo: Vec<String>,
        /// Override the primary Arch Linux pacman mirror URL
        #[arg(long, help_heading = "Packages & Repos")]
        pacman_mirror: Option<String>,

        // User & Access
        #[arg(long, action = clap::ArgAction::Append, help_heading = "User & Access")]
        group: Vec<String>,
        #[arg(long, help_heading = "User & Access")]
        shell: Option<String>,
        #[arg(long, help_heading = "User & Access")]
        sudo_nopasswd: bool,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "User & Access")]
        sudo_command: Vec<String>,

        // Firewall
        #[arg(long, help_heading = "Firewall")]
        firewall: bool,
        /// Firewall default policy, e.g. deny or reject.
        #[arg(long, help_heading = "Firewall")]
        firewall_policy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Firewall")]
        allow_port: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Firewall")]
        deny_port: Vec<String>,

        // Services
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Services")]
        enable_service: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Services")]
        disable_service: Vec<String>,

        // Containers
        #[arg(long, help_heading = "Containers")]
        docker: bool,
        #[arg(long, help_heading = "Containers")]
        podman: bool,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Containers")]
        docker_user: Vec<String>,

        // Storage & Encryption
        #[arg(long, help_heading = "Storage & Encryption")]
        swap_size: Option<u32>,
        #[arg(long, help_heading = "Storage & Encryption")]
        swap_file: Option<String>,
        #[arg(long, help_heading = "Storage & Encryption")]
        swappiness: Option<u8>,
        #[arg(long, help_heading = "Storage & Encryption")]
        encrypt: bool,
        #[arg(long, help_heading = "Storage & Encryption")]
        encrypt_passphrase: Option<String>,
        #[arg(long, help_heading = "Storage & Encryption")]
        encrypt_passphrase_file: Option<PathBuf>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Storage & Encryption")]
        mount: Vec<String>,

        // Boot
        #[arg(long, help_heading = "Boot")]
        grub_timeout: Option<u32>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Boot")]
        grub_cmdline: Vec<String>,
        #[arg(long, help_heading = "Boot")]
        grub_default: Option<String>,

        // Advanced
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Advanced")]
        sysctl: Vec<String>,
        /// Shell command to run AFTER install (in chroot). Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Advanced")]
        late_command: Vec<String>,
        /// Shell command to run BEFORE install. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Advanced")]
        run_command: Vec<String>,
        /// Finish without prompting during the target install when supported.
        #[arg(long, help_heading = "Advanced")]
        no_user_interaction: bool,
        /// Wallpaper asset to copy into supported desktop installers.
        #[arg(long, help_heading = "Advanced")]
        wallpaper: Option<PathBuf>,

        // Output
        /// Directory to write the output ISO into
        #[arg(long, help_heading = "Output")]
        out: PathBuf,
        /// Output ISO filename (e.g., my-server.iso)
        #[arg(long, help_heading = "Output")]
        name: Option<String>,
        #[arg(long, help_heading = "Output")]
        volume_label: Option<String>,
        /// Expected SHA-256 hex digest of the source ISO; operation aborts if it does not match.
        #[arg(long, help_heading = "Output")]
        expected_sha256: Option<String>,
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
                let (resolved_source, _preset_distro_tag) =
                    resolve_source_from_preset_or_str(source, preset)?;
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
            if !result.matched {
                std::process::exit(1);
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

            // Resolve source: --preset or --source
            let (resolved_source, preset_distro_tag) =
                resolve_source_from_preset_or_str(source, preset)?;

            // Parse distro — explicit --distro takes precedence; if omitted and a
            // preset was used, infer the distro from the preset's distro tag so that
            // e.g. `--preset rocky-linux` automatically selects the Kickstart path.
            let resolved_distro = match distro.as_deref() {
                Some("ubuntu") => None,
                Some("fedora") => Some(Distro::Fedora),
                Some("arch") => Some(Distro::Arch),
                Some("mint") => Some(Distro::Mint),
                Some(other) => {
                    eprintln!("ERROR: unknown distro '{other}'. Valid: ubuntu, fedora, arch, mint");
                    std::process::exit(1);
                }
                None => match preset_distro_tag {
                    Some("fedora") | Some("rhel-family") => Some(Distro::Fedora),
                    Some("arch") => Some(Distro::Arch),
                    Some("mint") => Some(Distro::Mint),
                    // Unsupported preset distros: warn and fall through to Ubuntu
                    // cloud-init (the user can override with --distro).
                    Some(tag @ ("debian" | "opensuse")) => {
                        eprintln!(
                            "WARNING: ForgeISO does not yet have a dedicated installer \
                             format for '{tag}'. Using Ubuntu cloud-init autoinstall as a \
                             best-effort fallback. The generated config may not work for \
                             this distro. Use --distro ubuntu to silence this warning, or \
                             see the ForgeISO docs for supported distros."
                        );
                        None
                    }
                    // ubuntu → cloud-init (no warning needed)
                    _ => None,
                },
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
/// Returns (source_url_or_path, preset_distro_tag).
/// `preset_distro_tag` is `Some("fedora")`, `Some("mint")`, etc. when a preset was
/// matched and `None` when the source was provided directly (user controls --distro).
fn resolve_source_from_preset_or_str(
    source: Option<String>,
    preset: Option<String>,
) -> anyhow::Result<(String, Option<&'static str>)> {
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
                Ok((url, Some(found.distro)))
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
        Ok((s, None))
    } else {
        anyhow::bail!("--source or --preset is required")
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_profile, resolve_source_from_preset_or_str, Cli, Commands};
    use clap::{CommandFactory, Subcommand};

    fn render_help(mut command: clap::Command) -> String {
        let mut buffer = Vec::new();
        command
            .write_long_help(&mut buffer)
            .expect("help should render");
        String::from_utf8(buffer).expect("help should be utf-8")
    }

    #[test]
    fn root_help_frames_guided_and_advanced_interfaces() {
        let help = render_help(Cli::command());
        assert!(help.contains("forgeiso-desktop"));
        assert!(help.contains("forgeiso-tui"));
        assert!(help.contains("Advanced CLI for local Linux ISO automation"));
    }

    #[test]
    fn inject_help_mentions_archinstall_and_valid_distros() {
        let inject = Commands::augment_subcommands(clap::Command::new("forgeiso"))
            .find_subcommand("inject")
            .expect("inject subcommand should exist")
            .clone();
        let help = render_help(inject);
        assert!(help.contains("distro-appropriate unattended install configuration"));
        assert!(help.contains("ubuntu"));
        assert!(help.contains("fedora"));
        assert!(help.contains("mint"));
        assert!(help.contains("arch"));
    }

    #[test]
    fn scan_help_uses_artifact_flag() {
        let scan = Commands::augment_subcommands(clap::Command::new("forgeiso"))
            .find_subcommand("scan")
            .expect("scan subcommand should exist")
            .clone();
        let help = render_help(scan);
        assert!(help.contains("--artifact"));
        assert!(!help.contains("--source"));
    }

    #[test]
    fn parse_profile_accepts_supported_profiles() {
        assert!(parse_profile("minimal").is_ok());
        assert!(parse_profile("desktop").is_ok());
        assert!(parse_profile("broken").is_err());
    }

    #[test]
    fn resolve_source_requires_source_or_preset() {
        let err =
            resolve_source_from_preset_or_str(None, None).expect_err("missing source must fail");
        assert!(err.to_string().contains("--source or --preset is required"));
    }
}
