use crate::config::{Distro, InjectConfig};
use crate::error::{EngineError, EngineResult};

use crate::autoinstall::{build_feature_late_commands, hash_password};

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
