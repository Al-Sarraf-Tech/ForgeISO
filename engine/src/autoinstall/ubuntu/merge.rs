use crate::autoinstall::{build_feature_late_commands, hash_password};
use crate::config::{Distro, InjectConfig};
use crate::error::{EngineError, EngineResult};

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
