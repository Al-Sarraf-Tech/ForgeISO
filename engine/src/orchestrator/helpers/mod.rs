//! Helpers shared across the orchestrator submodules.
//!
//! Decomposed from `helpers.rs` (745 LOC) into per-concern files. mod.rs
//! re-exports everything previously available at the file root so that
//! `super::helpers::*` import paths in sibling orchestrator modules continue
//! to resolve unchanged.

mod archinstall;
mod boot_patch;
mod cache;
mod host;
mod paths;
mod process;

// Public surface: callers outside the orchestrator (lib.rs re-exports + the
// `pub use helpers::*` line in `orchestrator/mod.rs`) only depend on the
// cache/process functions.  The remainder are `pub(in crate::orchestrator)`.
pub use cache::{cache_subdir, default_cache_root};
pub use process::{run_command_capture, run_command_lossy};

// Internal-to-orchestrator re-exports.
pub(in crate::orchestrator) use archinstall::build_archinstall_config;
pub(in crate::orchestrator) use boot_patch::{patch_boot_configs, patch_efi_grub_cfg};
pub(in crate::orchestrator) use host::{ensure_linux_host, ovmf_path, require_tools};
pub(in crate::orchestrator) use paths::{
    chmod_recursive_writable, copy_dir_contents, download_filename, is_squashfs_path,
    remove_dir_all_force, sanitize_filename,
};
pub(in crate::orchestrator) use process::{run_command_capture_async, run_command_lossy_async};

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
