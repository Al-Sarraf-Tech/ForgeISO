use crate::error::EngineResult;

/// Build a minimal archinstall JSON config from InjectConfig fields.
pub(in crate::orchestrator) fn build_archinstall_config(
    cfg: &crate::config::InjectConfig,
) -> EngineResult<serde_json::Value> {
    use serde_json::{json, Value};

    use crate::autoinstall::hash_password;

    // Build packages list: user-requested packages + container runtimes.
    // archinstall handles package installation from Arch repos; Docker CE
    // is available in the Arch community repo as "docker", and Podman as "podman".
    let mut pkg_list = cfg.extra_packages.clone();
    if cfg.containers.docker {
        pkg_list.push("docker".to_string());
        pkg_list.push("docker-compose".to_string());
    }
    if cfg.containers.podman {
        pkg_list.push("podman".to_string());
    }
    pkg_list.sort();
    pkg_list.dedup();
    let packages: Value = pkg_list.into();
    let services: Value = cfg.enable_services.to_vec().into();

    let mut map = serde_json::Map::new();
    if let Some(h) = &cfg.hostname {
        map.insert("hostname".to_string(), json!(h));
    }
    // ── User account ─────────────────────────────────────────────────────────
    // archinstall >= 2.7 prefers the "!users" list format which supports SSH keys,
    // sudo, shell, and other per-user options.  We also keep the legacy top-level
    // "username" / "!password" keys so older archinstall versions still work.
    if let Some(u) = &cfg.username {
        map.insert("username".to_string(), json!(u));

        let hashed = if let Some(p) = &cfg.password {
            hash_password(p)?
        } else {
            "!".to_string() // locked account placeholder
        };

        // Emit the !users list (archinstall >= 2.7 format).
        let mut user_obj = serde_json::Map::new();
        user_obj.insert("username".to_string(), json!(u));
        user_obj.insert("!password".to_string(), json!(hashed));
        user_obj.insert("sudo".to_string(), json!(true));
        if !cfg.ssh.authorized_keys.is_empty() {
            let keys: Vec<serde_json::Value> =
                cfg.ssh.authorized_keys.iter().map(|k| json!(k)).collect();
            user_obj.insert("ssh_authorized_keys".to_string(), json!(keys));
        }
        map.insert("!users".to_string(), json!([user_obj]));

        // Legacy top-level password field for archinstall < 2.7 compatibility.
        map.insert("!password".to_string(), json!(hashed));
    } else if let Some(p) = &cfg.password {
        let hashed = hash_password(p)?;
        map.insert("!password".to_string(), json!(hashed));
    }
    if let Some(tz) = &cfg.timezone {
        map.insert("timezone".to_string(), json!(tz));
    } else {
        map.insert("timezone".to_string(), json!("UTC"));
    }
    map.insert("mirror-region".to_string(), json!("Worldwide"));
    if let Some(loc) = &cfg.locale {
        map.insert("sys-language".to_string(), json!(loc));
    } else {
        map.insert("sys-language".to_string(), json!("en_US.UTF-8"));
    }
    if let Some(kb) = &cfg.keyboard_layout {
        map.insert("keyboard-layout".to_string(), json!(kb));
    } else {
        map.insert("keyboard-layout".to_string(), json!("us"));
    }
    map.insert("packages".to_string(), packages);
    map.insert("services".to_string(), services);
    map.insert("script".to_string(), json!("stealth-installation"));

    Ok(Value::Object(map))
}
