use std::path::PathBuf;

use forgeiso_engine::{
    ContainerConfig, Distro, FirewallConfig, ForgeIsoEngine, GrubConfig, InjectConfig, IsoSource,
    NetworkConfig, ProxyConfig, SshConfig, SwapConfig, UserConfig,
};

use crate::resolve_source_from_preset_or_str;

#[allow(clippy::too_many_arguments)]
pub async fn handle(
    engine: &ForgeIsoEngine,
    source: Option<String>,
    preset: Option<String>,
    autoinstall: Option<PathBuf>,
    out: PathBuf,
    name: Option<String>,
    volume_label: Option<String>,
    hostname: Option<String>,
    username: Option<String>,
    password: Option<String>,
    password_file: Option<PathBuf>,
    password_stdin: bool,
    realname: Option<String>,
    ssh_key: Vec<String>,
    ssh_key_file: Vec<PathBuf>,
    ssh_password_auth: bool,
    no_ssh_password_auth: bool,
    ssh_install_server: bool,
    no_ssh_install_server: bool,
    dns: Vec<String>,
    ntp_server: Vec<String>,
    timezone: Option<String>,
    locale: Option<String>,
    keyboard_layout: Option<String>,
    storage_layout: Option<String>,
    apt_mirror: Option<String>,
    package: Vec<String>,
    wallpaper: Option<PathBuf>,
    late_command: Vec<String>,
    run_command: Vec<String>,
    no_user_interaction: bool,
    group: Vec<String>,
    shell: Option<String>,
    sudo_nopasswd: bool,
    sudo_command: Vec<String>,
    firewall: bool,
    firewall_policy: Option<String>,
    allow_port: Vec<String>,
    deny_port: Vec<String>,
    static_ip: Option<String>,
    gateway: Option<String>,
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    no_proxy: Vec<String>,
    enable_service: Vec<String>,
    disable_service: Vec<String>,
    sysctl: Vec<String>,
    swap_size: Option<u32>,
    swap_file: Option<String>,
    swappiness: Option<u8>,
    apt_repo: Vec<String>,
    dnf_repo: Vec<String>,
    dnf_mirror: Option<String>,
    pacman_repo: Vec<String>,
    pacman_mirror: Option<String>,
    docker: bool,
    podman: bool,
    docker_user: Vec<String>,
    grub_timeout: Option<u32>,
    grub_cmdline: Vec<String>,
    grub_default: Option<String>,
    encrypt: bool,
    encrypt_passphrase: Option<String>,
    encrypt_passphrase_file: Option<PathBuf>,
    mount: Vec<String>,
    distro: Option<String>,
    expected_sha256: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
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
    let (resolved_source, preset_distro_tag) = resolve_source_from_preset_or_str(source, preset)?;

    // Parse distro -- explicit --distro takes precedence; if omitted and a
    // preset was used, infer the distro from the preset's distro tag so that
    // e.g. `--preset rocky-linux` automatically selects the Kickstart path.
    let resolved_distro = resolve_distro(distro.as_deref(), preset_distro_tag);

    // Parse sysctl "key=value" pairs -- warn on malformed entries
    let sysctl_pairs: Vec<(String, String)> = sysctl
        .iter()
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
                _ => {
                    eprintln!("WARNING: --sysctl {s:?} ignored (expected key=value format)");
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
    Ok(())
}

/// Resolve the target distro from an explicit --distro flag or a preset distro tag.
fn resolve_distro(
    explicit: Option<&str>,
    preset_distro_tag: Option<&'static str>,
) -> Option<Distro> {
    match explicit {
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
            // ubuntu -> cloud-init (no warning needed)
            _ => None,
        },
    }
}
