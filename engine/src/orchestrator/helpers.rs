use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{EngineError, EngineResult};

use super::CommandOutput;

pub fn default_cache_root() -> EngineResult<PathBuf> {
    if let Ok(path) = std::env::var("FORGEISO_CACHE_DIR") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }

    // XDG-compliant default: ~/.cache/forgeiso — avoids tmpfs quota issues and
    // the world-writable /tmp directory (which is susceptible to cache-poisoning
    // attacks on shared hosts).  If $HOME is unavailable the caller must provide
    // an explicit cache_dir instead of silently falling back to /tmp.
    let home = std::env::var("HOME").map_err(|_| {
        EngineError::InvalidConfig(
            "$HOME is not set; cannot determine default cache directory. \
             Set $HOME or provide an explicit --cache-dir"
                .to_string(),
        )
    })?;
    let path = PathBuf::from(home).join(".cache").join("forgeiso");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn cache_subdir(name: &str) -> EngineResult<PathBuf> {
    let path = default_cache_root()?.join(name);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn run_command_capture(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

    if !output.status.success() {
        return Err(EngineError::Runtime(format!(
            "{program} failed with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Like `run_command_capture` but tolerates non-zero exit codes (e.g. unsquashfs
/// returning exit 2 for device-node warnings when not running as root).
pub fn run_command_lossy(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

pub(super) async fn run_command_capture_async(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let program = program.to_string();
    let args = args.to_vec();
    let cwd = cwd.map(Path::to_path_buf);
    tokio::task::spawn_blocking(move || run_command_capture(&program, &args, cwd.as_deref()))
        .await
        .map_err(|e| EngineError::Runtime(format!("failed to join blocking task: {e}")))?
}

pub(super) async fn run_command_lossy_async(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let program = program.to_string();
    let args = args.to_vec();
    let cwd = cwd.map(Path::to_path_buf);
    tokio::task::spawn_blocking(move || run_command_lossy(&program, &args, cwd.as_deref()))
        .await
        .map_err(|e| EngineError::Runtime(format!("failed to join blocking task: {e}")))?
}

pub(super) fn ensure_linux_host() -> EngineResult<()> {
    if std::env::consts::OS != "linux" {
        return Err(EngineError::MissingTool(
            "ForgeISO local build/test is supported only on Linux hosts".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn require_tools(tools: &[&str]) -> EngineResult<()> {
    let missing = tools
        .iter()
        .filter(|tool| which::which(tool).is_err())
        .copied()
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(EngineError::MissingTool(format!(
            "missing local tools: {}",
            missing.join(", ")
        )))
    }
}

pub(super) fn is_squashfs_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".squashfs") || lower.ends_with(".sfs") || lower.ends_with(".erofs")
}

pub(super) fn download_filename(url: &str) -> String {
    let fallback = format!("download-{}.iso", chrono::Utc::now().timestamp());
    // Strip query string and fragment before extracting the path basename so
    // that URLs like ".../ubuntu.iso?token=abc" produce "ubuntu.iso" rather
    // than the mangled "ubuntu.iso-token-abc".
    let without_query = url.split_once('?').map_or(url, |(p, _)| p);
    let path_only = without_query
        .split_once('#')
        .map_or(without_query, |(p, _)| p);
    path_only
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename)
        .filter(|segment| !segment.is_empty())
        .unwrap_or(fallback)
}

pub(super) fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub(super) fn ovmf_path() -> EngineResult<PathBuf> {
    for candidate in [
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/edk2/x64/OVMF_CODE.fd",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(EngineError::MissingTool(
        "OVMF firmware is required for UEFI smoke tests".to_string(),
    ))
}

pub(super) fn copy_dir_contents(from: &Path, to: &Path) -> EngineResult<()> {
    for entry in WalkDir::new(from).into_iter().filter_map(Result::ok) {
        let relative = entry.path().strip_prefix(from).map_err(|e| {
            EngineError::Runtime(format!("failed to compute relative overlay path: {e}"))
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Recursively grant user-write permission before removal so files extracted
/// from ISOs (which may carry read-only permissions) can be deleted.
pub(super) fn remove_dir_all_force(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let Ok(meta) = entry.metadata() else { continue };
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(entry.path(), perms);
    }
    std::fs::remove_dir_all(path)
}

pub(super) fn chmod_recursive_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let Ok(meta) = entry.metadata() else { continue };
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(entry.path(), perms);
    }
}

/// Patch grub.cfg and isolinux.cfg boot entries with additional kernel params.
pub(super) fn patch_boot_configs(extract_dir: &Path, kernel_append: &str) -> EngineResult<()> {
    // Patch grub.cfg — try both canonical kernel paths.
    // Ubuntu live/desktop/server ISOs use /casper/vmlinuz since 20.04.
    // Older ISOs (pre-20.04) and some remasters use /boot/vmlinuz.
    // Both use a literal tab between the `linux` keyword and the path.
    let grub_path = extract_dir.join("boot").join("grub").join("grub.cfg");
    if grub_path.exists() {
        let content = std::fs::read_to_string(&grub_path)?;
        // Replace whichever pattern is present; only one will match per ISO.
        let patched = content
            .replace(
                "linux\t/casper/vmlinuz",
                &format!("linux\t/casper/vmlinuz{}", kernel_append),
            )
            .replace(
                "linux\t/boot/vmlinuz",
                &format!("linux\t/boot/vmlinuz{}", kernel_append),
            );
        std::fs::write(&grub_path, patched)?;
    }

    // Patch isolinux.cfg — the append line contains the full kernel cmdline;
    // /vmlinuz matches as a substring of /casper/vmlinuz and /boot/vmlinuz.
    let isolinux_path = extract_dir.join("isolinux").join("isolinux.cfg");
    if isolinux_path.exists() {
        let content = std::fs::read_to_string(&isolinux_path)?;
        let patched = content.replace("/vmlinuz", &format!("/vmlinuz{}", kernel_append));
        std::fs::write(&isolinux_path, patched)?;
    }

    Ok(())
}

/// Patch an EFI `grub.cfg` by appending `kernel_append` to every kernel
/// command line (`linuxefi` / `linux` lines).
///
/// A global `.replace("quiet", ...)` was used previously but is incorrect:
/// - It corrupts comments and menu labels containing the search word.
/// - It silently skips the injection when the word is absent (e.g. Fedora
///   ISOs that don't include `quiet` in their EFI config).
///
/// This function appends unconditionally to every `linuxefi` / `linux` line,
/// which is safe: duplicate or additional kernel parameters are ignored by the
/// bootloader.
pub(super) fn patch_efi_grub_cfg(content: &str, kernel_append: &str) -> String {
    let mut patched_lines: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("linuxefi ") || trimmed.starts_with("linux ") {
            patched_lines.push(format!("{}{}", line.trim_end(), kernel_append));
        } else {
            patched_lines.push(line.to_string());
        }
    }
    patched_lines.join("\n") + "\n"
}

/// Build a minimal archinstall JSON config from InjectConfig fields.
pub(super) fn build_archinstall_config(
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── download_filename ────────────────────────────────────────────────────

    #[test]
    fn download_filename_extracts_iso_basename() {
        let url = "https://releases.ubuntu.com/noble/ubuntu-24.04.1-live-server-amd64.iso";
        assert_eq!(
            download_filename(url),
            "ubuntu-24.04.1-live-server-amd64.iso"
        );
    }

    #[test]
    fn download_filename_strips_query_string() {
        let url = "https://cdn.example.com/ubuntu-24.04-live-server-amd64.iso?token=abc123&ttl=600";
        assert_eq!(
            download_filename(url),
            "ubuntu-24.04-live-server-amd64.iso",
            "query string must not bleed into filename"
        );
    }

    #[test]
    fn download_filename_strips_fragment() {
        let url = "https://cdn.example.com/fedora-40.iso#section";
        assert_eq!(
            download_filename(url),
            "fedora-40.iso",
            "fragment must not bleed into filename"
        );
    }

    #[test]
    fn download_filename_sanitizes_special_chars() {
        // Characters outside [a-zA-Z0-9._-] become '-'
        let url = "https://example.com/my%20file.iso";
        let name = download_filename(url);
        assert!(
            !name.contains('%'),
            "percent signs must be sanitized: {name}"
        );
        assert!(!name.is_empty(), "filename must not be empty");
    }

    #[test]
    fn download_filename_fallback_for_empty_segment() {
        // Trailing slash -> empty last segment -> fallback timestamp name
        let url = "https://example.com/";
        let name = download_filename(url);
        assert!(!name.is_empty(), "fallback must not be empty");
        assert!(
            name.ends_with(".iso"),
            "fallback should end with .iso: {name}"
        );
    }

    // ── sanitize_filename ────────────────────────────────────────────────────

    #[test]
    fn sanitize_filename_preserves_safe_chars() {
        assert_eq!(sanitize_filename("ubuntu-24.04.iso"), "ubuntu-24.04.iso");
    }

    #[test]
    fn sanitize_filename_replaces_unsafe_chars_with_dash() {
        let out = sanitize_filename("my file (v2).iso");
        assert!(!out.contains(' '), "spaces must be replaced: {out}");
        assert!(!out.contains('('), "parens must be replaced: {out}");
    }

    // ── patch_boot_configs ───────────────────────────────────────────────────

    #[test]
    fn patch_boot_configs_casper_path() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let grub_dir = tmp.path().join("boot").join("grub");
        std::fs::create_dir_all(&grub_dir).expect("create grub dir");
        // Ubuntu 22.04+ live ISO grub.cfg uses /casper/vmlinuz
        let grub_cfg = grub_dir.join("grub.cfg");
        std::fs::write(
            &grub_cfg,
            "linux\t/casper/vmlinuz quiet splash ---\ninitrd\t/casper/initrd\n",
        )
        .expect("write grub.cfg");

        patch_boot_configs(tmp.path(), " autoinstall ds=nocloud;s=/cdrom/nocloud/")
            .expect("patch should succeed");

        let content = std::fs::read_to_string(&grub_cfg).expect("read patched grub.cfg");
        assert!(
            content.contains("linux\t/casper/vmlinuz autoinstall ds=nocloud;s=/cdrom/nocloud/"),
            "casper vmlinuz line was not patched: {content:?}"
        );
    }

    #[test]
    fn patch_boot_configs_legacy_boot_path() {
        let tmp = tempfile::tempdir().expect("tmp dir");
        let grub_dir = tmp.path().join("boot").join("grub");
        std::fs::create_dir_all(&grub_dir).expect("create grub dir");
        // Older ISO grub.cfg uses /boot/vmlinuz
        let grub_cfg = grub_dir.join("grub.cfg");
        std::fs::write(
            &grub_cfg,
            "linux\t/boot/vmlinuz quiet splash\ninitrd\t/boot/initrd\n",
        )
        .expect("write grub.cfg");

        patch_boot_configs(tmp.path(), " autoinstall ds=nocloud;s=/cdrom/nocloud/")
            .expect("patch should succeed");

        let content = std::fs::read_to_string(&grub_cfg).expect("read patched grub.cfg");
        assert!(
            content.contains("linux\t/boot/vmlinuz autoinstall ds=nocloud;s=/cdrom/nocloud/"),
            "legacy vmlinuz line was not patched: {content:?}"
        );
    }

    // ── patch_efi_grub_cfg tests ─────────────────────────────────────────────

    #[test]
    fn patch_efi_grub_cfg_appends_to_linuxefi_line() {
        let input = "\
menuentry 'Fedora Linux' {\n\
  linuxefi /images/pxeboot/vmlinuz inst.stage2=hd:LABEL=Fedora quiet rhgb\n\
  initrdefi /images/pxeboot/initrd.img\n\
}\n";
        let patched = patch_efi_grub_cfg(input, " inst.ks=cdrom:/ks.cfg");
        assert!(
            patched.contains("linuxefi /images/pxeboot/vmlinuz inst.stage2=hd:LABEL=Fedora quiet rhgb inst.ks=cdrom:/ks.cfg"),
            "linuxefi line must have inst.ks appended: {patched:?}"
        );
        // menuentry and initrdefi lines must be unmodified
        assert!(
            patched.contains("menuentry 'Fedora Linux'"),
            "menuentry line must not be changed"
        );
        assert!(
            patched.contains("initrdefi /images/pxeboot/initrd.img"),
            "initrdefi line must not be changed"
        );
    }

    #[test]
    fn patch_efi_grub_cfg_works_without_quiet_keyword() {
        // Regression: the old .replace("quiet", ...) would silently skip injection
        // if a Fedora ISO doesn't contain "quiet" in its EFI grub.cfg.
        let input = "\
menuentry 'Fedora' {\n\
  linuxefi /vmlinuz inst.stage2=hd:LABEL=Fedora rhgb\n\
  initrdefi /initrd.img\n\
}\n";
        let patched = patch_efi_grub_cfg(input, " inst.ks=cdrom:/ks.cfg");
        assert!(
            patched.contains(
                "linuxefi /vmlinuz inst.stage2=hd:LABEL=Fedora rhgb inst.ks=cdrom:/ks.cfg"
            ),
            "inst.ks must be injected even without 'quiet': {patched:?}"
        );
    }

    #[test]
    fn patch_efi_grub_cfg_does_not_corrupt_comments() {
        // Regression: .replace("quiet splash", ...) would corrupt comments.
        let input = "\
# This entry boots quietly with a splash screen\n\
menuentry 'Mint' {\n\
  linuxefi /casper/vmlinuz boot=casper quiet splash\n\
}\n";
        let patched = patch_efi_grub_cfg(
            input,
            " auto=true priority=critical preseed/file=/cdrom/preseed.cfg",
        );
        // Comment must be unchanged
        assert!(
            patched.contains("# This entry boots quietly with a splash screen"),
            "comment line must not be modified: {patched:?}"
        );
        // Only the linuxefi line should have the preseed arg appended
        assert!(
            patched.contains("linuxefi /casper/vmlinuz boot=casper quiet splash auto=true"),
            "linuxefi line must have preseed args appended: {patched:?}"
        );
    }

    #[test]
    fn patch_efi_grub_cfg_handles_linux_lines_too() {
        // Some EFI configs use 'linux' instead of 'linuxefi' (systemd-boot style)
        let input = "  linux /vmlinuz root=/dev/sda1 quiet\n  initrd /initrd\n";
        let patched = patch_efi_grub_cfg(input, " inst.ks=cdrom:/ks.cfg");
        assert!(
            patched.contains("linux /vmlinuz root=/dev/sda1 quiet inst.ks=cdrom:/ks.cfg"),
            "linux (non-efi) line must also be patched: {patched:?}"
        );
        assert!(
            patched.contains("initrd /initrd"),
            "initrd line must not be changed"
        );
    }

    // ── build_archinstall_config ─────────────────────────────────────────────

    #[test]
    fn build_archinstall_config_hashes_password() {
        let cfg = crate::config::InjectConfig {
            password: Some("mysecret".to_string()),
            ..Default::default()
        };
        let val = build_archinstall_config(&cfg).expect("config");
        let pw = val
            .get("!password")
            .and_then(|v| v.as_str())
            .expect("!password key");
        // Should be a SHA-512-crypt hash, not the plaintext
        assert!(
            pw.starts_with("$6$"),
            "expected SHA-512 hash starting with $6$, got: {pw}"
        );
        assert_ne!(pw, "mysecret", "password must not be stored in plaintext");
    }

    #[test]
    fn build_archinstall_config_injects_ssh_keys() {
        use crate::config::{Distro, SshConfig};
        let key = "ssh-ed25519 AAAAC3Nz…arch-unit-key";
        let cfg = crate::config::InjectConfig {
            distro: Some(Distro::Arch),
            username: Some("archuser".to_string()),
            password: Some("APass1!".to_string()),
            ssh: SshConfig {
                authorized_keys: vec![key.to_string()],
                install_server: Some(true),
                allow_password_auth: Some(false),
            },
            ..Default::default()
        };
        let val = build_archinstall_config(&cfg).expect("config");

        // !users list must exist
        let users = val
            .get("!users")
            .and_then(|v| v.as_array())
            .expect("!users");
        assert_eq!(users.len(), 1, "exactly one user entry");

        let user = &users[0];
        assert_eq!(
            user.get("username").and_then(|v| v.as_str()),
            Some("archuser"),
            "username must match"
        );

        let keys = user
            .get("ssh_authorized_keys")
            .and_then(|v| v.as_array())
            .expect("ssh_authorized_keys must be present");
        assert_eq!(keys.len(), 1);
        assert_eq!(
            keys[0].as_str(),
            Some(key),
            "SSH key must appear verbatim in archinstall config"
        );

        // Password in !users must also be hashed
        let pw = user
            .get("!password")
            .and_then(|v| v.as_str())
            .expect("!password in user object");
        assert!(pw.starts_with("$6$"), "user password must be hashed");
    }

    #[test]
    fn arch_launcher_script_has_no_trailing_colon_in_config_arg() {
        // Regression: the run-archinstall.sh launcher had `--config "${CONFIG}:"`
        // (trailing colon). archinstall interprets that as a file path ending in `:`,
        // which does not exist, causing the installer to abort immediately.
        // The correct form is `--config "${CONFIG}"` (no trailing colon).
        let launcher = concat!(
            "#!/usr/bin/env bash\n",
            "# Generated by ForgeISO -- triggers archinstall in unattended mode\n",
            "set -euo pipefail\n",
            "CONFIG=\"/run/archiso/bootmnt/arch/boot/archinstall-config.json\"\n",
            "if [[ -f \"${CONFIG}\" ]]; then\n",
            "    archinstall --config \"${CONFIG}\" --silent\n",
            "else\n",
            "    echo \"ERROR: archinstall config not found at ${CONFIG}\" >&2\n",
            "    exit 1\n",
            "fi\n"
        );
        // The --config argument must not end with `:` before the closing quote.
        assert!(
            !launcher.contains("\"${CONFIG}:\""),
            "archinstall launcher must not pass a colon-suffixed path to --config; \
             archinstall would fail to open the config file"
        );
        // The corrected form must be present.
        assert!(
            launcher.contains("--config \"${CONFIG}\""),
            "archinstall --config must receive the bare path without a trailing colon"
        );
    }

    #[test]
    fn mint_preseed_contains_auto_params() {
        // The Mint boot patch appends preseed kernel params — verify the append string
        let kernel_append = " auto=true priority=critical preseed/file=/cdrom/preseed.cfg";
        assert!(kernel_append.contains("auto=true"));
        assert!(kernel_append.contains("preseed/file=/cdrom/preseed.cfg"));
    }

    // ── build_archinstall_config Docker/Podman packages ──────────────────────

    #[test]
    fn build_archinstall_config_includes_docker_package() {
        use crate::config::{ContainerConfig, Distro};
        let cfg = crate::config::InjectConfig {
            distro: Some(Distro::Arch),
            containers: ContainerConfig {
                docker: true,
                docker_users: vec![],
                podman: false,
            },
            ..Default::default()
        };
        let val = build_archinstall_config(&cfg).expect("config");
        let pkgs = val["packages"].as_array().expect("packages must be array");
        let pkg_names: Vec<&str> = pkgs.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            pkg_names.contains(&"docker"),
            "docker must be in archinstall packages: {pkg_names:?}"
        );
        assert!(
            pkg_names.contains(&"docker-compose"),
            "docker-compose must be in archinstall packages: {pkg_names:?}"
        );
    }

    #[test]
    fn build_archinstall_config_includes_podman_package() {
        use crate::config::{ContainerConfig, Distro};
        let cfg = crate::config::InjectConfig {
            distro: Some(Distro::Arch),
            containers: ContainerConfig {
                docker: false,
                docker_users: vec![],
                podman: true,
            },
            ..Default::default()
        };
        let val = build_archinstall_config(&cfg).expect("config");
        let pkgs = val["packages"].as_array().expect("packages must be array");
        let pkg_names: Vec<&str> = pkgs.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            pkg_names.contains(&"podman"),
            "podman must be in archinstall packages: {pkg_names:?}"
        );
    }
}
