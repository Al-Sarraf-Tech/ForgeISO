use crate::config::{Distro, InjectConfig};
use crate::error::{EngineError, EngineResult};

use super::{build_feature_late_commands, hash_password};

/// Generate a complete autoinstall YAML document from `InjectConfig`.
/// Returns a YAML string prefixed with `#cloud-config\n`.
#[allow(clippy::too_many_lines)]
#[allow(clippy::missing_errors_doc)]
pub fn generate_autoinstall_yaml(cfg: &InjectConfig) -> EngineResult<String> {
    let mut root = serde_yaml::Mapping::new();

    let mut autoinstall = serde_yaml::Mapping::new();
    let is_ubuntu_like = !matches!(cfg.distro, Some(Distro::Fedora | Distro::Arch));

    // version
    autoinstall.insert("version".into(), serde_yaml::Value::Number(1.into()));

    // locale
    let locale = cfg.locale.as_deref().unwrap_or("en_US.UTF-8");
    autoinstall.insert(
        "locale".into(),
        serde_yaml::Value::String(locale.to_string()),
    );

    // keyboard
    let mut keyboard = serde_yaml::Mapping::new();
    keyboard.insert(
        "layout".into(),
        serde_yaml::Value::String(cfg.keyboard_layout.as_deref().unwrap_or("us").to_string()),
    );
    autoinstall.insert("keyboard".into(), serde_yaml::Value::Mapping(keyboard));

    // timezone
    let timezone = cfg.timezone.as_deref().unwrap_or("UTC");
    autoinstall.insert(
        "timezone".into(),
        serde_yaml::Value::String(timezone.to_string()),
    );

    // identity (if hostname or username is set)
    if cfg.hostname.is_some()
        || cfg.username.is_some()
        || cfg.password.is_some()
        || cfg.realname.is_some()
    {
        let mut identity = serde_yaml::Mapping::new();
        identity.insert(
            "hostname".into(),
            serde_yaml::Value::String(cfg.hostname.as_deref().unwrap_or("ubuntu").to_string()),
        );
        identity.insert(
            "username".into(),
            serde_yaml::Value::String(cfg.username.as_deref().unwrap_or("ubuntu").to_string()),
        );

        if let Some(pwd) = &cfg.password {
            let hashed = hash_password(pwd)?;
            identity.insert("password".into(), serde_yaml::Value::String(hashed));
        }

        if let Some(realname) = &cfg.realname {
            identity.insert(
                "realname".into(),
                serde_yaml::Value::String(realname.clone()),
            );
        }

        autoinstall.insert("identity".into(), serde_yaml::Value::Mapping(identity));
    }

    // SSH
    let mut ssh = serde_yaml::Mapping::new();

    // install-server defaults to true.  The server must be installed
    // regardless of whether the user provides authorized_keys — if keys are
    // configured the server is needed to accept them; if only password auth is
    // used the server is needed for that too.  The caller can opt out by
    // explicitly setting `install_server = Some(false)`.
    let install_server = cfg.ssh.install_server.unwrap_or(true);
    ssh.insert(
        "install-server".into(),
        serde_yaml::Value::Bool(install_server),
    );

    // authorized-keys
    if !cfg.ssh.authorized_keys.is_empty() {
        let keys: Vec<serde_yaml::Value> = cfg
            .ssh
            .authorized_keys
            .iter()
            .map(|k| serde_yaml::Value::String(k.clone()))
            .collect();
        ssh.insert("authorized-keys".into(), serde_yaml::Value::Sequence(keys));
    }

    // allow-pw: false if keys present, else true (unless explicitly set)
    let allow_pw = cfg
        .ssh
        .allow_password_auth
        .unwrap_or(cfg.ssh.authorized_keys.is_empty());
    ssh.insert("allow-pw".into(), serde_yaml::Value::Bool(allow_pw));

    autoinstall.insert("ssh".into(), serde_yaml::Value::Mapping(ssh));

    // network (static IP or DNS servers)
    if cfg.static_ip.is_some() || !cfg.network.dns_servers.is_empty() {
        let mut network = serde_yaml::Mapping::new();
        network.insert("version".into(), serde_yaml::Value::Number(2.into()));

        let mut ethernets = serde_yaml::Mapping::new();
        let mut any = serde_yaml::Mapping::new();

        let mut match_obj = serde_yaml::Mapping::new();
        match_obj.insert("name".into(), serde_yaml::Value::String("en*".to_string()));
        any.insert("match".into(), serde_yaml::Value::Mapping(match_obj));

        if let Some(static_ip) = &cfg.static_ip {
            any.insert("dhcp4".into(), serde_yaml::Value::Bool(false));
            let addresses = vec![serde_yaml::Value::String(static_ip.clone())];
            any.insert("addresses".into(), serde_yaml::Value::Sequence(addresses));

            if let Some(gateway) = &cfg.gateway {
                let mut routes = serde_yaml::Sequence::new();
                let mut route = serde_yaml::Mapping::new();
                route.insert(
                    "to".into(),
                    serde_yaml::Value::String("default".to_string()),
                );
                route.insert("via".into(), serde_yaml::Value::String(gateway.clone()));
                routes.push(serde_yaml::Value::Mapping(route));
                any.insert("routes".into(), serde_yaml::Value::Sequence(routes));
            }
        } else {
            any.insert("dhcp4".into(), serde_yaml::Value::Bool(true));
        }

        if !cfg.network.dns_servers.is_empty() {
            let mut nameservers = serde_yaml::Mapping::new();
            let addrs: Vec<serde_yaml::Value> = cfg
                .network
                .dns_servers
                .iter()
                .map(|d| serde_yaml::Value::String(d.clone()))
                .collect();
            nameservers.insert("addresses".into(), serde_yaml::Value::Sequence(addrs));
            any.insert(
                "nameservers".into(),
                serde_yaml::Value::Mapping(nameservers),
            );
        }

        ethernets.insert("any".into(), serde_yaml::Value::Mapping(any));
        network.insert("ethernets".into(), serde_yaml::Value::Mapping(ethernets));

        autoinstall.insert("network".into(), serde_yaml::Value::Mapping(network));
    }

    // storage — ALWAYS included for fully unattended install.
    // Without a storage.layout, Subiquity pauses and prompts the user.
    {
        let layout_name = cfg.storage_layout.as_deref().unwrap_or("lvm").to_string();
        let mut storage = serde_yaml::Mapping::new();
        let mut layout_map = serde_yaml::Mapping::new();
        layout_map.insert("name".into(), serde_yaml::Value::String(layout_name));
        if cfg.encrypt {
            if let Some(passphrase) = &cfg.encrypt_passphrase {
                // NOTE: Ubuntu cloud-init autoinstall requires the LUKS passphrase in
                // plaintext — there is no pre-hashing option for the storage.layout
                // password field. The caller must treat this ISO as sensitive material
                // and restrict access accordingly (chmod 600, encrypted transport, etc.).
                layout_map.insert(
                    "password".into(),
                    serde_yaml::Value::String(passphrase.clone()),
                );
            }
        }
        storage.insert("layout".into(), serde_yaml::Value::Mapping(layout_map));
        autoinstall.insert("storage".into(), serde_yaml::Value::Mapping(storage));
    }

    // apt (only if apt_mirror set)
    if is_ubuntu_like {
        if let Some(mirror) = &cfg.apt_mirror {
            let mut apt = serde_yaml::Mapping::new();
            let mut primary_seq = serde_yaml::Sequence::new();
            let mut primary_entry = serde_yaml::Mapping::new();

            // Use ["default"] so the entry applies to all architectures (amd64, arm64, etc.).
            // Hardcoding ["amd64"] would cause cloud-init to silently skip this entry on
            // non-amd64 systems, leaving the apt_mirror setting with no effect.
            let arches: serde_yaml::Sequence =
                vec![serde_yaml::Value::String("default".to_string())];
            primary_entry.insert("arches".into(), serde_yaml::Value::Sequence(arches));

            primary_entry.insert("uri".into(), serde_yaml::Value::String(mirror.clone()));

            primary_seq.push(serde_yaml::Value::Mapping(primary_entry));
            apt.insert("primary".into(), serde_yaml::Value::Sequence(primary_seq));

            autoinstall.insert("apt".into(), serde_yaml::Value::Mapping(apt));
        }
    }

    // packages (with auto-added feature packages)
    let mut all_packages = cfg.extra_packages.clone();
    if cfg.wallpaper.is_some() {
        all_packages.push("dconf-cli".to_string());
    }
    if cfg.firewall.enabled && is_ubuntu_like {
        all_packages.push("ufw".to_string());
    }
    if cfg.containers.podman {
        all_packages.push("podman".to_string());
    }
    if is_ubuntu_like && cfg.apt_repos.iter().any(|r| r.starts_with("ppa:")) {
        all_packages.push("software-properties-common".to_string());
    }
    all_packages.sort();
    all_packages.dedup();

    if !all_packages.is_empty() {
        let pkgs: Vec<serde_yaml::Value> = all_packages
            .iter()
            .map(|p| serde_yaml::Value::String(p.clone()))
            .collect();
        autoinstall.insert("packages".into(), serde_yaml::Value::Sequence(pkgs));
    }

    // late-commands (using feature helper)
    let late_commands = build_feature_late_commands(cfg)?;

    if !late_commands.is_empty() {
        let cmds: Vec<serde_yaml::Value> = late_commands
            .iter()
            .map(|c| serde_yaml::Value::String(c.clone()))
            .collect();
        autoinstall.insert("late-commands".into(), serde_yaml::Value::Sequence(cmds));
    }

    // interactive-sections (only if no_user_interaction = true)
    if cfg.no_user_interaction {
        autoinstall.insert(
            "interactive-sections".into(),
            serde_yaml::Value::Sequence(vec![]),
        );
    }

    root.insert(
        "autoinstall".into(),
        serde_yaml::Value::Mapping(autoinstall),
    );

    // Serialize and prepend cloud-config header.
    // We build only the `autoinstall:` root key; the `#cloud-config` line is a
    // cloud-init directive prepended directly rather than inserted as a YAML key
    // (inserting it as YAML then filtering by substring was fragile — any string
    // value containing "cloud-config:" would have been incorrectly removed).
    let yaml_str = serde_yaml::to_string(&root)
        .map_err(|e| EngineError::Runtime(format!("Failed to serialize YAML: {e}")))?;

    Ok(format!("#cloud-config\n{yaml_str}"))
}

/// Merge `InjectConfig` into an existing autoinstall YAML string.
/// CLI config fields override YAML fields. late-commands are appended, packages/keys are merged.
#[allow(clippy::too_many_lines)]
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
pub fn merge_autoinstall_yaml(existing: &str, cfg: &InjectConfig) -> EngineResult<String> {
    let is_ubuntu_like = !matches!(cfg.distro, Some(Distro::Fedora | Distro::Arch));

    // Parse existing YAML
    let mut root: serde_yaml::Value = serde_yaml::from_str(existing)
        .map_err(|e| EngineError::Runtime(format!("Failed to parse YAML: {e}")))?;

    // Get or create autoinstall mapping
    let autoinstall_map = if let Some(ai) = root.get_mut("autoinstall") {
        ai.as_mapping_mut()
            .ok_or_else(|| EngineError::Runtime("autoinstall must be a mapping".to_string()))?
    } else {
        // Create new autoinstall entry
        let mut new_root = serde_yaml::Mapping::new();
        new_root.insert(
            "autoinstall".into(),
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
        );
        root = serde_yaml::Value::Mapping(new_root);
        root.get_mut("autoinstall")
            .expect("just inserted autoinstall key")
            .as_mapping_mut()
            .expect("just inserted autoinstall as Mapping")
    };

    // Override scalar fields from cfg
    if let Some(locale) = &cfg.locale {
        autoinstall_map.insert("locale".into(), serde_yaml::Value::String(locale.clone()));
    }

    if let Some(timezone) = &cfg.timezone {
        autoinstall_map.insert(
            "timezone".into(),
            serde_yaml::Value::String(timezone.clone()),
        );
    }

    // keyboard
    if cfg.keyboard_layout.is_some() {
        let mut keyboard = autoinstall_map
            .remove("keyboard")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();
        keyboard.insert(
            "layout".into(),
            serde_yaml::Value::String(cfg.keyboard_layout.as_deref().unwrap_or("us").to_string()),
        );
        autoinstall_map.insert("keyboard".into(), serde_yaml::Value::Mapping(keyboard));
    }

    // identity
    if cfg.hostname.is_some()
        || cfg.username.is_some()
        || cfg.password.is_some()
        || cfg.realname.is_some()
    {
        let mut identity = autoinstall_map
            .remove("identity")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        if let Some(hostname) = &cfg.hostname {
            identity.insert(
                "hostname".into(),
                serde_yaml::Value::String(hostname.clone()),
            );
        }

        if let Some(username) = &cfg.username {
            identity.insert(
                "username".into(),
                serde_yaml::Value::String(username.clone()),
            );
        }

        if let Some(password) = &cfg.password {
            let hashed = hash_password(password)?;
            identity.insert("password".into(), serde_yaml::Value::String(hashed));
        }

        if let Some(realname) = &cfg.realname {
            identity.insert(
                "realname".into(),
                serde_yaml::Value::String(realname.clone()),
            );
        }

        autoinstall_map.insert("identity".into(), serde_yaml::Value::Mapping(identity));
    }

    // SSH
    if !cfg.ssh.authorized_keys.is_empty()
        || cfg.ssh.allow_password_auth.is_some()
        || cfg.ssh.install_server.is_some()
    {
        let mut ssh = autoinstall_map
            .remove("ssh")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        if !cfg.ssh.authorized_keys.is_empty() {
            let keys: Vec<serde_yaml::Value> = cfg
                .ssh
                .authorized_keys
                .iter()
                .map(|k| serde_yaml::Value::String(k.clone()))
                .collect();
            ssh.insert("authorized-keys".into(), serde_yaml::Value::Sequence(keys));
        }

        if let Some(allow_pw) = cfg.ssh.allow_password_auth {
            ssh.insert("allow-pw".into(), serde_yaml::Value::Bool(allow_pw));
        }

        if let Some(install) = cfg.ssh.install_server {
            ssh.insert("install-server".into(), serde_yaml::Value::Bool(install));
        }

        autoinstall_map.insert("ssh".into(), serde_yaml::Value::Mapping(ssh));
    }

    // network (static IP or DNS)
    // NTP servers are NOT written to the netplan block; they go to
    // systemd-timesyncd.conf via build_feature_late_commands().
    // Omitting the ntp_servers check here prevents an empty `network: {}`
    // block from being injected into the YAML when only NTP is configured.
    if cfg.static_ip.is_some() || !cfg.network.dns_servers.is_empty() {
        let mut network = autoinstall_map
            .remove("network")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        network.insert("version".into(), serde_yaml::Value::Number(2.into()));
        let mut ethernets = serde_yaml::Mapping::new();
        let mut any = serde_yaml::Mapping::new();

        let mut match_obj = serde_yaml::Mapping::new();
        match_obj.insert("name".into(), serde_yaml::Value::String("en*".to_string()));
        any.insert("match".into(), serde_yaml::Value::Mapping(match_obj));

        if let Some(static_ip) = &cfg.static_ip {
            any.insert("dhcp4".into(), serde_yaml::Value::Bool(false));
            let addresses = vec![serde_yaml::Value::String(static_ip.clone())];
            any.insert("addresses".into(), serde_yaml::Value::Sequence(addresses));

            if let Some(gateway) = &cfg.gateway {
                let mut routes = serde_yaml::Sequence::new();
                let mut route = serde_yaml::Mapping::new();
                route.insert(
                    "to".into(),
                    serde_yaml::Value::String("default".to_string()),
                );
                route.insert("via".into(), serde_yaml::Value::String(gateway.clone()));
                routes.push(serde_yaml::Value::Mapping(route));
                any.insert("routes".into(), serde_yaml::Value::Sequence(routes));
            }
        } else {
            any.insert("dhcp4".into(), serde_yaml::Value::Bool(true));
        }

        if !cfg.network.dns_servers.is_empty() {
            let mut nameservers = serde_yaml::Mapping::new();
            let addrs: Vec<serde_yaml::Value> = cfg
                .network
                .dns_servers
                .iter()
                .map(|d| serde_yaml::Value::String(d.clone()))
                .collect();
            nameservers.insert("addresses".into(), serde_yaml::Value::Sequence(addrs));

            any.insert(
                "nameservers".into(),
                serde_yaml::Value::Mapping(nameservers),
            );
        }

        ethernets.insert("any".into(), serde_yaml::Value::Mapping(any));
        network.insert("ethernets".into(), serde_yaml::Value::Mapping(ethernets));

        autoinstall_map.insert("network".into(), serde_yaml::Value::Mapping(network));
    }

    // storage — ALWAYS included for fully unattended install.
    {
        let layout_name = cfg.storage_layout.as_deref().unwrap_or("lvm").to_string();
        let mut storage = autoinstall_map
            .remove("storage")
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();
        let mut layout_map = serde_yaml::Mapping::new();
        layout_map.insert("name".into(), serde_yaml::Value::String(layout_name));
        if cfg.encrypt {
            if let Some(passphrase) = &cfg.encrypt_passphrase {
                // NOTE: Ubuntu cloud-init autoinstall requires the LUKS passphrase in
                // plaintext — there is no pre-hashing option for the storage.layout
                // password field. The caller must treat this ISO as sensitive material
                // and restrict access accordingly (chmod 600, encrypted transport, etc.).
                layout_map.insert(
                    "password".into(),
                    serde_yaml::Value::String(passphrase.clone()),
                );
            }
        }
        storage.insert("layout".into(), serde_yaml::Value::Mapping(layout_map));
        autoinstall_map.insert("storage".into(), serde_yaml::Value::Mapping(storage));
    }

    // apt
    if is_ubuntu_like {
        if let Some(mirror) = &cfg.apt_mirror {
            let mut apt = autoinstall_map
                .remove("apt")
                .and_then(|v| v.as_mapping().cloned())
                .unwrap_or_default();
            let mut primary_seq = serde_yaml::Sequence::new();
            let mut primary_entry = serde_yaml::Mapping::new();

            // Use ["default"] so the entry applies to all architectures (amd64, arm64, etc.).
            let arches: serde_yaml::Sequence =
                vec![serde_yaml::Value::String("default".to_string())];
            primary_entry.insert("arches".into(), serde_yaml::Value::Sequence(arches));

            primary_entry.insert("uri".into(), serde_yaml::Value::String(mirror.clone()));

            primary_seq.push(serde_yaml::Value::Mapping(primary_entry));
            apt.insert("primary".into(), serde_yaml::Value::Sequence(primary_seq));

            autoinstall_map.insert("apt".into(), serde_yaml::Value::Mapping(apt));
        }
    }

    // packages: merge (auto-add + dedup)
    let mut all_packages = cfg.extra_packages.clone();
    if cfg.wallpaper.is_some() {
        all_packages.push("dconf-cli".to_string());
    }
    if cfg.firewall.enabled && is_ubuntu_like {
        all_packages.push("ufw".to_string());
    }
    if cfg.containers.podman {
        all_packages.push("podman".to_string());
    }
    if is_ubuntu_like && cfg.apt_repos.iter().any(|r| r.starts_with("ppa:")) {
        all_packages.push("software-properties-common".to_string());
    }

    if let Some(existing_pkgs) = autoinstall_map
        .get("packages")
        .and_then(|v| v.as_sequence())
    {
        for pkg_val in existing_pkgs {
            if let Some(pkg_str) = pkg_val.as_str() {
                all_packages.push(pkg_str.to_string());
            }
        }
    }

    all_packages.sort();
    all_packages.dedup();

    if !all_packages.is_empty() {
        let pkgs: Vec<serde_yaml::Value> = all_packages
            .iter()
            .map(|p| serde_yaml::Value::String(p.clone()))
            .collect();
        autoinstall_map.insert("packages".into(), serde_yaml::Value::Sequence(pkgs));
    }

    // late-commands: existing + new features (appended)
    let mut all_late_commands = Vec::new();

    // Existing commands
    if let Some(existing_cmds) = autoinstall_map
        .get("late-commands")
        .and_then(|v| v.as_sequence())
    {
        for cmd_val in existing_cmds {
            if let Some(cmd_str) = cmd_val.as_str() {
                all_late_commands.push(cmd_str.to_string());
            }
        }
    }

    // Append all feature late-commands
    all_late_commands.extend(build_feature_late_commands(cfg)?);
    let mut deduped_late_commands = Vec::with_capacity(all_late_commands.len());
    for command in all_late_commands {
        if !deduped_late_commands.contains(&command) {
            deduped_late_commands.push(command);
        }
    }

    if !deduped_late_commands.is_empty() {
        let cmds: Vec<serde_yaml::Value> = deduped_late_commands
            .iter()
            .map(|c: &String| serde_yaml::Value::String(c.clone()))
            .collect();
        autoinstall_map.insert("late-commands".into(), serde_yaml::Value::Sequence(cmds));
    }

    // interactive-sections
    if cfg.no_user_interaction {
        autoinstall_map.insert(
            "interactive-sections".into(),
            serde_yaml::Value::Sequence(vec![]),
        );
    }

    // Serialize back
    let yaml_str = serde_yaml::to_string(&root)
        .map_err(|e| EngineError::Runtime(format!("Failed to serialize YAML: {e}")))?;

    // Preserve cloud-config header if original had it
    if existing.starts_with("#cloud-config") {
        Ok(format!("#cloud-config\n{yaml_str}"))
    } else {
        Ok(yaml_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ContainerConfig, Distro, FirewallConfig, GrubConfig, IsoSource, NetworkConfig, ProxyConfig,
        SshConfig, UserConfig,
    };

    #[test]
    fn test_generate_minimal_yaml() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.starts_with("#cloud-config"),
            "YAML should start with #cloud-config"
        );
        assert!(
            yaml.contains("autoinstall:"),
            "YAML should contain autoinstall section"
        );
        assert!(
            yaml.contains("version: 1"),
            "YAML should contain version: 1"
        );
    }

    #[test]
    fn run_command_containing_cloud_config_substring_is_not_filtered() {
        // Regression: the old implementation filtered lines by substring match
        // `!line.contains("cloud-config:")`. Any late-command string value whose
        // YAML serialisation contained that substring would be silently dropped,
        // producing a YAML with a missing late-command.
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            run_commands: vec!["echo 'cloud-config: done'".to_string()],
            ..Default::default()
        };
        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("cloud-config: done"),
            "run_command containing 'cloud-config:' must not be filtered from YAML: {yaml}"
        );
        assert!(
            yaml.starts_with("#cloud-config"),
            "YAML must still start with #cloud-config header"
        );
    }

    #[test]
    fn test_generate_with_identity() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: Some("test-host".to_string()),
            username: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            realname: Some("Test User".to_string()),
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("identity:"),
            "YAML should contain identity section"
        );
        assert!(yaml.contains("test-host"), "hostname should be in YAML");
        assert!(yaml.contains("testuser"), "username should be in YAML");
        assert!(yaml.contains("$6$"), "password should be hashed with $6$");
        assert!(yaml.contains("Test User"), "realname should be in YAML");
    }

    #[test]
    fn test_generate_with_ssh_keys() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: crate::config::SshConfig {
                authorized_keys: vec![
                    "ssh-ed25519 AAAA...".to_string(),
                    "ssh-rsa BBBB...".to_string(),
                ],
                allow_password_auth: None,
                install_server: None,
            },
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("ssh:"), "YAML should contain ssh section");
        assert!(yaml.contains("AAAA"), "first key should be in YAML");
        assert!(yaml.contains("BBBB"), "second key should be in YAML");
        assert!(
            yaml.contains("allow-pw: false"),
            "allow-pw should be false when keys present"
        );
        // Regression: the old default was `authorized_keys.is_empty()` which
        // evaluated to `false` when keys were provided, setting install-server
        // to false and making the authorized keys unusable (no SSH daemon).
        assert!(
            yaml.contains("install-server: true"),
            "install-server must default to true even when authorized_keys are provided: {yaml}"
        );
    }

    #[test]
    fn test_generate_with_dns() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: crate::config::NetworkConfig {
                dns_servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
                ntp_servers: vec![],
            },
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("network:"),
            "YAML should contain network section"
        );
        assert!(yaml.contains("1.1.1.1"), "DNS 1 should be in YAML");
        assert!(yaml.contains("8.8.8.8"), "DNS 2 should be in YAML");
    }

    #[test]
    fn test_generate_with_wallpaper() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: Some(std::path::PathBuf::from("/tmp/bg.jpg")),
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("late-commands:"),
            "YAML should contain late-commands"
        );
        assert!(
            yaml.contains("cp /cdrom/wallpaper/bg.jpg"),
            "copy command should be present"
        );
        assert!(
            yaml.contains("dconf update"),
            "dconf update should be present"
        );
        assert!(
            yaml.contains("dconf-cli"),
            "dconf-cli should be in packages"
        );
    }

    #[test]
    fn test_merge_preserves_existing() {
        let existing = r"
autoinstall:
  version: 1
  storage:
    layout:
      name: lvm
";
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: Some("newhost".to_string()),
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
        assert!(
            result.contains("lvm"),
            "existing storage layout should be preserved"
        );
        assert!(result.contains("newhost"), "new hostname should be present");
    }

    #[test]
    fn test_merge_overrides_identity() {
        let existing = r"
autoinstall:
  identity:
    username: olduser
    hostname: oldhost
";
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: Some("newhost".to_string()),
            username: Some("newuser".to_string()),
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
        assert!(result.contains("newuser"), "new username should override");
        assert!(result.contains("newhost"), "new hostname should override");
        assert!(!result.contains("olduser"), "old username should be gone");
        assert!(!result.contains("oldhost"), "old hostname should be gone");
    }

    #[test]
    fn test_merge_appends_late_commands() {
        let existing = r#"
autoinstall:
  late-commands:
    - "echo existing"
"#;
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec!["echo new".to_string()],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let result = merge_autoinstall_yaml(existing, &cfg).unwrap();
        assert!(
            result.contains("echo existing"),
            "existing command should be preserved"
        );
        assert!(
            result.contains("echo new"),
            "new command should be appended"
        );
    }

    #[test]
    fn test_generate_with_user_groups() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: Some("testuser".to_string()),
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: crate::config::UserConfig {
                groups: vec!["sudo".to_string(), "docker".to_string()],
                shell: None,
                sudo_nopasswd: false,
                sudo_commands: vec![],
            },
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("usermod -aG sudo,docker testuser"),
            "usermod command should add groups"
        );
    }

    #[test]
    fn test_generate_with_sudo_nopasswd() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: Some("testuser".to_string()),
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: crate::config::UserConfig {
                groups: vec![],
                shell: None,
                sudo_nopasswd: true,
                sudo_commands: vec![],
            },
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("NOPASSWD:ALL"),
            "sudo NOPASSWD should be configured"
        );
        assert!(yaml.contains("chmod 440"), "sudoers file permissions");
    }

    #[test]
    fn test_generate_with_firewall() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: crate::config::FirewallConfig {
                enabled: true,
                default_policy: Some("deny".to_string()),
                allow_ports: vec!["22".to_string(), "443".to_string()],
                deny_ports: vec![],
            },
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("ufw"), "firewall package should be added");
        assert!(yaml.contains("ufw --force enable"), "ufw enable command");
        assert!(yaml.contains("ufw allow 22"), "allow port 22");
    }

    #[test]
    fn test_generate_with_static_ip() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: Some("10.0.0.5/24".to_string()),
            gateway: Some("10.0.0.1".to_string()),
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("dhcp4: false"),
            "static IP should disable DHCP"
        );
        assert!(yaml.contains("10.0.0.5/24"), "static IP should be present");
        assert!(yaml.contains("10.0.0.1"), "gateway should be present");
    }

    #[test]
    fn test_generate_with_proxy() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: crate::config::ProxyConfig {
                http_proxy: Some("http://proxy.example.com:8080".to_string()),
                https_proxy: Some("http://proxy.example.com:8443".to_string()),
                no_proxy: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            },
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("http_proxy"), "http_proxy in environment");
        assert!(yaml.contains("Acquire::http::Proxy"), "apt http proxy");
        assert!(yaml.contains("no_proxy"), "no_proxy in environment");
    }

    #[test]
    fn test_generate_with_services() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec!["nginx".to_string()],
            disable_services: vec!["bluetooth".to_string()],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("systemctl enable nginx"), "enable nginx");
        assert!(
            yaml.contains("systemctl disable bluetooth"),
            "disable bluetooth"
        );
    }

    #[test]
    fn test_generate_with_sysctl() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![
                ("vm.swappiness".to_string(), "10".to_string()),
                ("net.ipv4.ip_forward".to_string(), "1".to_string()),
            ],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("vm.swappiness=10"), "sysctl setting");
        assert!(
            yaml.contains("sysctl.d/99-forgeiso.conf"),
            "sysctl config file"
        );
    }

    #[test]
    fn test_generate_with_swap() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: Some(crate::config::SwapConfig {
                size_mb: 4096,
                filename: Some("/swapfile".to_string()),
                swappiness: Some(10),
            }),
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("fallocate -l 4096M"), "swap allocation");
        assert!(yaml.contains("mkswap"), "swap mkswap");
        assert!(yaml.contains("/etc/fstab"), "fstab entry");
    }

    #[test]
    fn test_generate_with_docker() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: Some("admin".to_string()),
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: crate::config::ContainerConfig {
                docker: true,
                podman: false,
                docker_users: vec!["admin".to_string()],
            },
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("docker-ce"), "docker packages");
        assert!(yaml.contains("download.docker.com"), "docker repo");
        assert!(yaml.contains("usermod -aG docker admin"), "docker user");
    }

    #[test]
    fn test_generate_with_grub() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: crate::config::GrubConfig {
                timeout: Some(5),
                cmdline_extra: vec!["quiet".to_string(), "iommu=on".to_string()],
                default_entry: None,
            },
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("GRUB_TIMEOUT=5"), "grub timeout");
        assert!(yaml.contains("update-grub"), "update-grub command");
    }

    #[test]
    fn test_generate_with_mounts() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: None,
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: false,
            encrypt_passphrase: None,
            mounts: vec!["/dev/sdb1 /data ext4 defaults 0 2".to_string()],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(yaml.contains("mkdir -p /target/data"), "create mount point");
        assert!(yaml.contains("/dev/sdb1 /data"), "fstab entry");
    }

    #[test]
    fn test_generate_with_encryption() {
        let cfg = InjectConfig {
            source: crate::config::IsoSource::from_raw("/tmp/test.iso"),
            autoinstall_yaml: None,
            out_name: "out.iso".to_string(),
            output_label: None,
            expected_sha256: None,
            hostname: None,
            username: None,
            password: None,
            realname: None,
            ssh: SshConfig::default(),
            network: NetworkConfig::default(),
            timezone: None,
            locale: None,
            keyboard_layout: None,
            storage_layout: Some("lvm".to_string()),
            apt_mirror: None,
            extra_packages: vec![],
            wallpaper: None,
            extra_late_commands: vec![],
            no_user_interaction: false,
            user: UserConfig::default(),
            firewall: FirewallConfig::default(),
            proxy: ProxyConfig::default(),
            static_ip: None,
            gateway: None,
            enable_services: vec![],
            disable_services: vec![],
            sysctl: vec![],
            swap: None,
            apt_repos: vec![],
            containers: ContainerConfig::default(),
            grub: GrubConfig::default(),
            encrypt: true,
            encrypt_passphrase: Some("secret".to_string()),
            mounts: vec![],
            run_commands: vec![],
            distro: None,
            dnf_repos: vec![],
            dnf_mirror: None,
            pacman_repos: vec![],
            pacman_mirror: None,
        };

        let yaml = generate_autoinstall_yaml(&cfg).unwrap();
        assert!(
            yaml.contains("password:"),
            "encryption password in storage section"
        );
        assert!(yaml.contains("secret"), "passphrase should be in YAML");
    }

    // ── merge_autoinstall_yaml edge cases ─────────────────────────────────────

    #[test]
    fn merge_autoinstall_yaml_with_no_autoinstall_key_creates_it() {
        // YAML that has a version key but NO autoinstall: key.
        // merge_autoinstall_yaml must create the autoinstall section rather than error.
        let bare = "version: 1\nidentity:\n  hostname: old\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            hostname: Some("new-host".to_string()),
            ..Default::default()
        };
        let result = merge_autoinstall_yaml(bare, &cfg);
        assert!(result.is_ok(), "must not error on missing autoinstall key");
        let yaml = result.unwrap();
        assert!(
            yaml.contains("autoinstall"),
            "autoinstall key must be created"
        );
        assert!(yaml.contains("new-host"), "new hostname must appear");
    }

    #[test]
    fn merge_autoinstall_yaml_with_empty_input_creates_valid_yaml() {
        // Completely empty string is valid YAML (null document).
        // The function should create a minimal autoinstall section.
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            locale: Some("en_US.UTF-8".to_string()),
            ..Default::default()
        };
        let result = merge_autoinstall_yaml("", &cfg);
        assert!(result.is_ok(), "empty YAML must not error");
    }

    #[test]
    fn merge_autoinstall_yaml_malformed_input_returns_error() {
        // Tabs at column 0 are illegal in YAML — must return Err, not panic.
        let bad = "\t\tinvalid: [yaml\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            ..Default::default()
        };
        let result = merge_autoinstall_yaml(bad, &cfg);
        assert!(result.is_err(), "malformed YAML must return an error");
    }

    #[test]
    fn merge_autoinstall_yaml_preserves_cloud_config_header() {
        let existing = "#cloud-config\nautoinstall:\n  version: 1\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            locale: Some("en_GB.UTF-8".to_string()),
            ..Default::default()
        };
        let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
        assert!(
            yaml.starts_with("#cloud-config"),
            "cloud-config header must be preserved"
        );
        assert!(yaml.contains("en_GB.UTF-8"));
    }

    #[test]
    fn merge_autoinstall_yaml_appends_to_existing_late_commands() {
        let existing = "autoinstall:\n  version: 1\n  late-commands:\n    - echo first\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            network: NetworkConfig {
                ntp_servers: vec!["time.cloudflare.com".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
        assert!(
            yaml.contains("echo first"),
            "original late-command preserved"
        );
        assert!(yaml.contains("timesyncd"), "new NTP late-command appended");
    }

    #[test]
    fn merge_autoinstall_yaml_deduplicates_packages() {
        // The existing YAML already contains "curl"; cfg also adds "curl".
        // After merge, "curl" must appear exactly once.
        let existing = "autoinstall:\n  version: 1\n  packages:\n    - curl\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            extra_packages: vec!["curl".to_string(), "git".to_string()],
            ..Default::default()
        };
        let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
        let curl_count = yaml.matches("curl").count();
        assert_eq!(curl_count, 1, "curl must appear exactly once after dedup");
        assert!(yaml.contains("git"), "git must appear in merged packages");
    }

    #[test]
    fn merge_autoinstall_ntp_only_does_not_inject_empty_network_block() {
        // Regression: merge_autoinstall_yaml previously entered the network block
        // when only NTP servers were configured, inserting an empty `network: {}`
        // into the YAML.  NTP goes to systemd-timesyncd via late-commands only;
        // it must not touch the netplan network block.
        let existing = "autoinstall:\n  version: 1\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            network: NetworkConfig {
                ntp_servers: vec!["time.cloudflare.com".to_string()],
                dns_servers: vec![],
            },
            static_ip: None,
            ..Default::default()
        };
        let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
        assert!(
            !yaml.contains("network:"),
            "no netplan network block should appear when only NTP is configured: {yaml}"
        );
        // NTP must still appear in the late-commands section.
        assert!(
            yaml.contains("timesyncd"),
            "NTP config must still be written to late-commands: {yaml}"
        );
    }

    #[test]
    fn apt_mirror_uses_default_arches_not_amd64() {
        // Regression: arches was hardcoded to ["amd64"], causing cloud-init to silently
        // skip the apt primary entry on arm64 and other architectures.  Must be ["default"].
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            apt_mirror: Some("http://mirror.example.com/ubuntu".to_string()),
            ..Default::default()
        };
        let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
        assert!(
            yaml.contains("default"),
            "apt primary arches must be 'default', not 'amd64': {yaml}"
        );
        assert!(
            !yaml.contains("amd64"),
            "apt primary arches must not be hardcoded to 'amd64': {yaml}"
        );
    }

    #[test]
    fn merge_apt_mirror_uses_default_arches_not_amd64() {
        // Same regression as above but exercised via merge_autoinstall_yaml.
        let existing = "autoinstall:\n  version: 1\n";
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            apt_mirror: Some("http://mirror.example.com/ubuntu".to_string()),
            ..Default::default()
        };
        let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
        assert!(
            yaml.contains("default"),
            "merged apt primary arches must be 'default', not 'amd64': {yaml}"
        );
        assert!(
            !yaml.contains("amd64"),
            "merged apt primary arches must not be hardcoded to 'amd64': {yaml}"
        );
    }

    #[test]
    fn arch_generate_with_firewall_does_not_add_ufw_package() {
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            distro: Some(Distro::Arch),
            firewall: FirewallConfig {
                enabled: true,
                default_policy: Some("deny".to_string()),
                allow_ports: vec!["22".to_string()],
                deny_ports: vec![],
            },
            ..Default::default()
        };

        let yaml = generate_autoinstall_yaml(&cfg).expect("generate must succeed");
        assert!(
            !yaml.contains("ufw"),
            "Arch cloud-init fallback must not inject Ubuntu-specific ufw package: {yaml}"
        );
    }

    #[test]
    fn merge_autoinstall_yaml_deduplicates_existing_late_commands() {
        let existing = concat!(
            "autoinstall:\n",
            "  version: 1\n",
            "  late-commands:\n",
            "    - chroot /target systemctl enable systemd-timesyncd\n"
        );
        let cfg = InjectConfig {
            source: IsoSource::from_raw("/tmp/test.iso"),
            out_name: "out.iso".to_string(),
            network: NetworkConfig {
                ntp_servers: vec!["time.cloudflare.com".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let yaml = merge_autoinstall_yaml(existing, &cfg).expect("merge must succeed");
        assert_eq!(
            yaml.matches("chroot /target systemctl enable systemd-timesyncd")
                .count(),
            1,
            "merge must keep existing late-commands stable instead of duplicating them: {yaml}"
        );
    }
}
