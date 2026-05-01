//! Internal validators for [`super::InjectConfig`].
//!
//! The public entry point is [`super::InjectConfig::validate`], which
//! orchestrates the per-concern validators in this module. Each validator
//! is intentionally narrow: it inspects only the fields it needs and
//! returns the same [`EngineError::InvalidConfig`] variants as the
//! original monolithic implementation. Behaviour is preserved exactly.

use crate::error::{EngineError, EngineResult};

use super::super::components::{
    ContainerConfig, FirewallConfig, GrubConfig, NetworkConfig, ProxyConfig, SshConfig, SwapConfig,
    UserConfig,
};
use super::super::validation::{
    is_safe_cidr, is_safe_identifier, is_safe_network_addr, is_safe_path, is_safe_port,
};
use super::InjectConfig;

use std::path::PathBuf;

// -- Identity / locale -------------------------------------------------------

pub(super) fn validate_identity(
    hostname: Option<&String>,
    username: Option<&String>,
    realname: Option<&String>,
    timezone: Option<&String>,
    locale: Option<&String>,
    keyboard_layout: Option<&String>,
) -> EngineResult<()> {
    if let Some(h) = hostname {
        is_safe_identifier(h, "hostname")?;
    }
    if let Some(u) = username {
        is_safe_identifier(u, "username")?;
    }

    // Timezone -- written as a bare string into cloud-init YAML, Kickstart
    // `timezone` directive, and preseed `time/zone`.  Only IANA-style chars
    // are valid (e.g. "America/New_York", "UTC", "Etc/GMT+5").  Block
    // everything that is not alphanumeric, slash, underscore, dash, or plus.
    if let Some(tz) = timezone {
        if tz.is_empty() {
            return Err(EngineError::InvalidConfig(
                "timezone must not be blank".to_string(),
            ));
        }
        if !tz
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "timezone contains unsafe characters: {tz:?} \
                 (only alphanumeric, slash, underscore, dash, plus allowed)"
            )));
        }
    }

    // Locale -- written as a bare string into cloud-init YAML and installer
    // directives.  Standard glibc locale names use alphanumeric, dash,
    // underscore, and dot (e.g. "en_US.UTF-8", "de_DE.ISO-8859-1").
    if let Some(loc) = locale {
        if loc.is_empty() {
            return Err(EngineError::InvalidConfig(
                "locale must not be blank".to_string(),
            ));
        }
        if !loc
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "locale contains unsafe characters: {loc:?} \
                 (only alphanumeric, underscore, dash, dot allowed)"
            )));
        }
    }

    // Keyboard layout -- written into cloud-init YAML keyboard.layout.
    // XKB layout identifiers are alphanumeric plus dash and underscore
    // (e.g. "us", "de", "gb", "us-intl").
    if let Some(kb) = keyboard_layout {
        if kb.is_empty() {
            return Err(EngineError::InvalidConfig(
                "keyboard_layout must not be blank".to_string(),
            ));
        }
        if !kb
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "keyboard_layout contains unsafe characters: {kb:?} \
                 (only alphanumeric, dash, underscore allowed)"
            )));
        }
    }
    if let Some(r) = realname {
        // Realname can contain spaces
        if r.chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "realname contains shell metacharacters: {r:?}"
            )));
        }
    }

    Ok(())
}

// -- Users / services / firewall / sysctl / sudo -----------------------------

pub(super) fn validate_user_basics(user: &UserConfig) -> EngineResult<()> {
    for g in &user.groups {
        is_safe_identifier(g, "group")?;
    }
    if let Some(shell) = &user.shell {
        is_safe_path(shell, "shell")?;
    }
    Ok(())
}

pub(super) fn validate_user_sudo(user: &UserConfig) -> EngineResult<()> {
    // Sudo commands -- these are written into sudoers, so block metacharacters
    // that could break sudoers syntax or inject shell commands.
    for cmd in &user.sudo_commands {
        if cmd
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "sudo_command contains shell metacharacters: {cmd:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_services(
    enable_services: &[String],
    disable_services: &[String],
) -> EngineResult<()> {
    for svc in enable_services {
        is_safe_identifier(svc, "enable_service")?;
    }
    for svc in disable_services {
        is_safe_identifier(svc, "disable_service")?;
    }
    Ok(())
}

pub(super) fn validate_firewall(firewall: &FirewallConfig) -> EngineResult<()> {
    if let Some(policy) = &firewall.default_policy {
        is_safe_identifier(policy, "firewall_policy")?;
    }
    for port in &firewall.allow_ports {
        is_safe_port(port, "allow_port")?;
    }
    for port in &firewall.deny_ports {
        is_safe_port(port, "deny_port")?;
    }
    Ok(())
}

pub(super) fn validate_sysctl(sysctl: &[(String, String)]) -> EngineResult<()> {
    for (key, val) in sysctl {
        is_safe_identifier(key, "sysctl key")?;
        // Sysctl values can be numeric or simple strings
        if val
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "sysctl value contains shell metacharacters: {val:?}"
            )));
        }
    }
    Ok(())
}

// -- Packages / repos / mirrors ----------------------------------------------

pub(super) fn validate_apt_repos(apt_repos: &[String]) -> EngineResult<()> {
    // APT repos -- written via echo into sources.list files
    for repo in apt_repos {
        if repo
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "apt_repo contains shell metacharacters: {repo:?}"
            )));
        }
        // Enforce apt sources.list line format -- allow deb/deb-src lines and
        // ppa: shorthand (handled via add-apt-repository in generated late-commands).
        let trimmed = repo.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with("deb ")
            && !trimmed.starts_with("deb-src ")
            && !trimmed.starts_with("ppa:")
        {
            return Err(EngineError::InvalidConfig(format!(
                "apt_repo must be a 'deb '/'deb-src ' sources.list entry or a 'ppa:' \
                 shorthand: {repo:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_apt_mirror(apt_mirror: Option<&String>) -> EngineResult<()> {
    // APT mirror -- used in YAML and potentially late-commands
    if let Some(mirror) = apt_mirror {
        if mirror
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "apt_mirror contains shell metacharacters: {mirror:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_dnf(dnf_repos: &[String], dnf_mirror: Option<&String>) -> EngineResult<()> {
    // DNF mirror -- interpolated into: sed -i 's|^baseurl=.*|baseurl={mirror}/...|'
    // The `|` character is the sed delimiter so it must be blocked to prevent
    // the substitution from being split or manipulated.  Newlines and null bytes
    // would also break the sed one-liner or produce invalid output.
    if let Some(mirror) = dnf_mirror {
        if mirror.contains('|') || mirror.contains('\n') || mirror.contains('\r') {
            return Err(EngineError::InvalidConfig(format!(
                "dnf_mirror must not contain `|` (sed delimiter) or newlines: {mirror:?}"
            )));
        }
        if mirror.contains('\0') {
            return Err(EngineError::InvalidConfig(
                "dnf_mirror must not contain a null byte".to_string(),
            ));
        }
    }

    // DNF repos -- two write paths exist in kickstart.rs:
    //   URL entries  -> single-quoted:  dnf config-manager --add-repo '...'
    //   Stanza entries -> heredoc:      cat > /etc/yum.repos.d/... << 'FORGEISO_REPO_EOF'
    // For URL entries a literal ' would break out of the single-quoted argument,
    // so we block it.  Stanza entries use a heredoc with a fixed sentinel so they
    // are safe against all shell metacharacters; only null bytes are rejected below.
    for repo in dnf_repos {
        let trimmed = repo.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            // Single-quote injection risk in the URL path.
            if trimmed.contains('\'') {
                return Err(EngineError::InvalidConfig(format!(
                    "dnf_repo URL contains a single quote: {repo:?}"
                )));
            }
        }
        // Both paths: null bytes and raw control chars (other than \n / \t in
        // stanzas) would produce invalid output.
        if trimmed.contains('\0') {
            return Err(EngineError::InvalidConfig(format!(
                "dnf_repo contains a null byte: {repo:?}"
            )));
        }
        // Stanza path: the heredoc sentinel must not appear as a standalone
        // line in the stanza -- it would terminate the heredoc early and
        // produce a truncated .repo file.
        for line in repo.lines() {
            if line.trim() == "FORGEISO_REPO_EOF" {
                return Err(EngineError::InvalidConfig(
                    "dnf_repo stanza must not contain a line that is exactly \
                     'FORGEISO_REPO_EOF' (heredoc sentinel collision)"
                        .to_string(),
                ));
            }
        }
    }

    Ok(())
}

pub(super) fn validate_pacman(
    pacman_repos: &[String],
    pacman_mirror: Option<&String>,
) -> EngineResult<()> {
    // Pacman mirror -- written as: echo 'Server = {mirror}/$repo/os/$arch' >
    // In a single-quoted shell string $ and other metacharacters are literal
    // and safe; only a ' itself can break out of the quoting.
    if let Some(mirror) = pacman_mirror {
        if mirror.contains('\'') || mirror.contains('\n') || mirror.contains('\r') {
            return Err(EngineError::InvalidConfig(format!(
                "pacman_mirror must not contain single quotes or newlines: {mirror:?}"
            )));
        }
    }

    // Pacman repos -- each entry written via: echo '{line}' >> mirrorlist
    // Same single-quote injection risk; newlines would break the echo command.
    for repo in pacman_repos {
        if repo.contains('\'') || repo.contains('\n') || repo.contains('\r') {
            return Err(EngineError::InvalidConfig(format!(
                "pacman_repo must not contain single quotes or newlines: {repo:?}"
            )));
        }
    }

    Ok(())
}

pub(super) fn validate_packages(extra_packages: &[String]) -> EngineResult<()> {
    // extra_packages -- each entry is written as a bare line in Kickstart
    // %packages, interpolated into Mint preseed `pkgsel/include`, or
    // serialised into cloud-init YAML.  In Kickstart, a package name
    // containing a newline followed by `%end` would terminate the
    // %packages section early and allow injecting arbitrary directives.
    // Valid dpkg/rpm/pacman package names use alphanumeric, dash,
    // underscore, dot, plus, and colon (architecture qualifier).
    for pkg in extra_packages {
        if pkg.is_empty() {
            return Err(EngineError::InvalidConfig(
                "extra_packages entry must not be empty".to_string(),
            ));
        }
        if !pkg
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | ':'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "extra_packages entry contains unsafe characters: {pkg:?} \
                 (only alphanumeric, dash, underscore, dot, plus, colon allowed)"
            )));
        }
    }

    Ok(())
}

// -- Network -----------------------------------------------------------------

pub(super) fn validate_proxy(proxy: &ProxyConfig) -> EngineResult<()> {
    // Proxy URLs -- written to /etc/environment via echo
    for (field, val) in [
        ("http_proxy", &proxy.http_proxy),
        ("https_proxy", &proxy.https_proxy),
    ] {
        if let Some(url) = val {
            if url
                .chars()
                .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "{field} contains shell metacharacters: {url:?}"
                )));
            }
        }
    }

    // no_proxy entries -- written to /etc/environment
    for entry in &proxy.no_proxy {
        if entry
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "no_proxy contains shell metacharacters: {entry:?}"
            )));
        }
    }

    Ok(())
}

pub(super) fn validate_network(
    network: &NetworkConfig,
    static_ip: Option<&String>,
    gateway: Option<&String>,
) -> EngineResult<()> {
    // Static IP -- CIDR notation (e.g. "192.168.1.10/24") placed in cloud-init
    // netplan YAML, Kickstart `--ip=`, and preseed `netcfg/get_ipaddress`.
    if let Some(ip) = static_ip {
        is_safe_cidr(ip, "static_ip")?;
    }

    // Gateway -- plain IP or hostname placed in cloud-init routes and Kickstart
    // `--gateway=` directive.
    if let Some(gw) = gateway {
        is_safe_network_addr(gw, "gateway")?;
    }

    // DNS servers -- may be IPv4, IPv6, or hostnames.
    for dns in &network.dns_servers {
        is_safe_network_addr(dns, "dns_server")?;
    }

    // NTP servers -- may be IPv4, IPv6, or hostnames.
    for ntp in &network.ntp_servers {
        is_safe_network_addr(ntp, "ntp_server")?;
    }

    Ok(())
}

// -- SSH / containers --------------------------------------------------------

pub(super) fn validate_ssh(ssh: &SshConfig) -> EngineResult<()> {
    // SSH authorized_keys -- for Mint distro, each key is written via:
    //   printf '%s\n' 'KEY_CONTENT' >> .../authorized_keys
    // The key content is single-quoted so $ and ` are literal.  A single
    // quote (') inside the key content would break out of the quoting and
    // allow arbitrary shell injection.  Valid SSH public keys never contain
    // single quotes (base64 alphabet is A-Z a-z 0-9 + / =; the optional
    // comment field should not contain shell metacharacters), so this check
    // only rejects malformed or malicious input.
    //
    // The FORGEISO_KEY_EOF sentinel check is kept as defense in depth even
    // though the heredoc approach is no longer used -- any future code that
    // reintroduces a heredoc for these keys would be protected.
    for key in &ssh.authorized_keys {
        if key.contains('\'') {
            return Err(EngineError::InvalidConfig(
                "authorized_key must not contain a single quote ('): \
                 single-quoted shell argument would be broken"
                    .to_string(),
            ));
        }
        // Double-quote check: the Kickstart `sshkey` directive wraps the key
        // in double quotes (`sshkey --username=user "KEY"`).  A `"` in the key
        // comment would terminate the quoting early and allow injection.
        if key.contains('"') {
            return Err(EngineError::InvalidConfig(
                "authorized_key must not contain a double quote (\"): \
                 double-quoted Kickstart sshkey argument would be broken"
                    .to_string(),
            ));
        }
        // Newlines: SSH authorized_keys entries are single-line; an embedded
        // newline would break both the Kickstart sshkey directive (line-oriented)
        // and the preseed late_command (which is also a single-line shell string).
        if key.contains('\n') || key.contains('\r') {
            return Err(EngineError::InvalidConfig(
                "authorized_key must not contain a newline: \
                 each key must be a single line"
                    .to_string(),
            ));
        }
        for line in key.lines() {
            if line.trim() == "FORGEISO_KEY_EOF" {
                return Err(EngineError::InvalidConfig(
                    "authorized_key must not contain a line that is exactly \
                     'FORGEISO_KEY_EOF' (heredoc sentinel collision)"
                        .to_string(),
                ));
            }
        }
    }

    Ok(())
}

pub(super) fn validate_containers(containers: &ContainerConfig) -> EngineResult<()> {
    for u in &containers.docker_users {
        is_safe_identifier(u, "docker_user")?;
    }
    Ok(())
}

// -- Storage / encryption / mounts / wallpaper -------------------------------

pub(super) fn validate_swap(swap: Option<&SwapConfig>) -> EngineResult<()> {
    // Swap filename
    // The filename is interpolated as:
    //   fallocate -l {mb}M /target{fname}   -> requires leading / to produce /target/swapfile
    //   chroot /target mkswap {fname}        -> requires absolute path inside the chroot
    //   echo '{fname} none swap ...' >> fstab -> requires absolute path
    // A relative name like "myswap" would create /targetmyswap (no separator),
    // and mkswap/fstab would reference a relative path that doesn't exist.
    if let Some(swap) = swap {
        if swap.size_mb == 0 {
            return Err(EngineError::InvalidConfig(
                "swap.size_mb must be greater than 0".to_string(),
            ));
        }
        if let Some(v) = swap.swappiness {
            if v > 100 {
                return Err(EngineError::InvalidConfig(format!(
                    "swap.swappiness must be 0\u{2013}100, got {v}"
                )));
            }
        }
        if let Some(fname) = &swap.filename {
            is_safe_path(fname, "swap_filename")?;
            if !fname.starts_with('/') {
                return Err(EngineError::InvalidConfig(format!(
                    "swap_filename must be an absolute path starting with '/': {fname:?}"
                )));
            }
            // Block .. path components: fallocate and chmod are called as
            // `command /target{fname}` so a traversal like `/../etc/passwd`
            // would resolve to /etc/passwd on the installer's running system.
            if fname.split('/').any(|c| c == "..") {
                return Err(EngineError::InvalidConfig(format!(
                    "swap_filename must not contain '..' path traversal: {fname:?}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_swap_size(swap: Option<&SwapConfig>) -> EngineResult<()> {
    // Swap size upper bound -- accepting arbitrarily large values (e.g. 999 GB)
    // would not fail validation but would produce a swap file that can never be
    // allocated, causing the installer to hang or error at runtime.
    // Cap at 128 GB (131072 MB), which is larger than any reasonable swap need.
    if let Some(swap) = swap {
        if swap.size_mb > 131_072 {
            return Err(EngineError::InvalidConfig(format!(
                "swap.size_mb {} exceeds maximum of 131072 (128 GiB)",
                swap.size_mb
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_mounts(mounts: &[String]) -> EngineResult<()> {
    // Mount entries -- written into fstab via echo
    for entry in mounts {
        if entry
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "mount entry contains shell metacharacters: {entry:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_encryption(
    encrypt: bool,
    encrypt_passphrase: Option<&String>,
    storage_layout: Option<&String>,
) -> EngineResult<()> {
    // Encryption: a passphrase is required when encrypt=true.
    // cloud-init autoinstall requires storage.layout.password; without it
    // the installer fails or silently uses an empty LUKS passphrase, which
    // is a serious security defect. There is no interactive fallback in
    // unattended mode.
    if encrypt && encrypt_passphrase.is_none() {
        return Err(EngineError::InvalidConfig(
            "encrypt is enabled but no encrypt_passphrase was provided; \
             Ubuntu cloud-init requires a LUKS passphrase in the storage layout"
                .to_string(),
        ));
    }

    // Encryption also requires a storage_layout -- without one, the autoinstall
    // YAML has no storage.layout block, so the LUKS password has nowhere to go
    // and encryption is silently skipped by cloud-init.
    if encrypt && storage_layout.is_none() {
        return Err(EngineError::InvalidConfig(
            "encrypt is enabled but no storage_layout was provided; \
             Ubuntu cloud-init requires a named storage layout (e.g. 'lvm' or 'direct') \
             to attach the LUKS passphrase to"
                .to_string(),
        ));
    }

    Ok(())
}

pub(super) fn validate_wallpaper(wallpaper: Option<&PathBuf>) -> EngineResult<()> {
    // Wallpaper -- the filename component is used directly in an unquoted shell
    // `cp /cdrom/wallpaper/{filename}` command.  A malicious filename like
    // `foo; rm -rf /.jpg` would execute arbitrary code on the installer's
    // running system.  Apply the same character set as is_safe_path: only
    // alphanumeric, dash, underscore, dot, and plus are allowed.
    if let Some(wp) = wallpaper {
        if let Some(fname) = wp.file_name().and_then(|n| n.to_str()) {
            if fname
                .chars()
                .any(|c| !c.is_alphanumeric() && !matches!(c, '-' | '_' | '.' | '+'))
            {
                return Err(EngineError::InvalidConfig(format!(
                    "wallpaper filename contains unsafe characters: {fname:?} \
                     (only alphanumeric, dash, underscore, dot, plus allowed)"
                )));
            }
        } else {
            return Err(EngineError::InvalidConfig(
                "wallpaper path must have a valid UTF-8 filename component".to_string(),
            ));
        }
    }
    Ok(())
}

// -- GRUB --------------------------------------------------------------------

pub(super) fn validate_grub(grub: &GrubConfig) -> EngineResult<()> {
    // GRUB -- default_entry and cmdline_extra are interpolated into sed s|...|...|
    // patterns (| delimiter).  Block shell metacharacters and | itself, but
    // allow / so users can specify UUID paths (e.g. rd.luks.uuid=/dev/sda2).
    if let Some(entry) = &grub.default_entry {
        if entry
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "grub_default contains shell metacharacters: {entry:?}"
            )));
        }
    }
    for param in &grub.cmdline_extra {
        if param
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "grub_cmdline contains shell metacharacters: {param:?}"
            )));
        }
    }
    // GRUB timeout -- written as a number into GRUB_TIMEOUT=N; unreasonably
    // large values produce unusable systems.  Cap at 3600 (1 hour).
    if let Some(t) = grub.timeout {
        if t > 3600 {
            return Err(EngineError::InvalidConfig(format!(
                "grub_timeout must be 0\u{2013}3600 seconds, got {t}"
            )));
        }
    }
    Ok(())
}

// -- Output / labels / hashes ------------------------------------------------

pub(super) fn validate_output_label(output_label: Option<&String>) -> EngineResult<()> {
    // output_label -- used as the ISO volume label (written to xorriso -V).
    // Must follow the same rules as BuildConfig: non-empty, <= 32 ASCII chars.
    if let Some(label) = output_label {
        let label = label.trim();
        if label.is_empty() {
            return Err(EngineError::InvalidConfig(
                "output_label must not be blank".to_string(),
            ));
        }
        if label.len() > 32 {
            return Err(EngineError::InvalidConfig(format!(
                "output_label is too long ({} chars, max 32)",
                label.len()
            )));
        }
        if !label.is_ascii() {
            return Err(EngineError::InvalidConfig(
                "output_label must contain only ASCII characters".to_string(),
            ));
        }
        if label.chars().any(|c| c.is_ascii_control()) {
            return Err(EngineError::InvalidConfig(
                "output_label must not contain control characters".to_string(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_out_name(out_name: &str) -> EngineResult<()> {
    // out_name -- used as a filename component joined with the output directory.
    // Block path separators (/ and \) to prevent writing outside the workspace.
    if !out_name.trim().is_empty() {
        let name = out_name.trim();
        if name.contains('/') || name.contains('\\') {
            return Err(EngineError::InvalidConfig(format!(
                "out_name must be a plain filename, not a path: {name:?}"
            )));
        }
        // Also block shell metacharacters in case the name is passed to xorriso.
        if name
            .chars()
            .any(|c| matches!(c, ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\n'))
        {
            return Err(EngineError::InvalidConfig(format!(
                "out_name contains shell metacharacters: {name:?}"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_sha256(expected_sha256: Option<&String>) -> EngineResult<()> {
    // expected_sha256 -- must be exactly 64 lowercase hex characters if provided.
    // A non-hex value would cause a confusing "SHA-256 mismatch" error at
    // download time rather than a clear "invalid format" error at config time.
    if let Some(sha) = expected_sha256 {
        let sha = sha.trim().to_ascii_lowercase();
        if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(EngineError::InvalidConfig(format!(
                "expected_sha256 must be a 64-character hex string, got {:?} ({} chars)",
                sha,
                sha.len()
            )));
        }
    }
    Ok(())
}

// -- Top-level orchestrator --------------------------------------------------

/// Mirrors the exact validation order of the original monolithic
/// `InjectConfig::validate()`: identity → user.basics → services →
/// firewall → sysctl → user.sudo → apt_repos → mounts → apt_mirror →
/// proxy → network (static_ip/gateway/dns/ntp) → ssh → containers →
/// swap → output_label → wallpaper → grub → out_name → dnf → pacman →
/// sha256 → swap_size → encryption → packages.
pub(super) fn run(cfg: &InjectConfig) -> EngineResult<()> {
    validate_identity(
        cfg.hostname.as_ref(),
        cfg.username.as_ref(),
        cfg.realname.as_ref(),
        cfg.timezone.as_ref(),
        cfg.locale.as_ref(),
        cfg.keyboard_layout.as_ref(),
    )?;

    validate_user_basics(&cfg.user)?;
    validate_services(&cfg.enable_services, &cfg.disable_services)?;
    validate_firewall(&cfg.firewall)?;
    validate_sysctl(&cfg.sysctl)?;
    validate_user_sudo(&cfg.user)?;

    validate_apt_repos(&cfg.apt_repos)?;
    validate_mounts(&cfg.mounts)?;
    validate_apt_mirror(cfg.apt_mirror.as_ref())?;

    validate_proxy(&cfg.proxy)?;
    validate_network(&cfg.network, cfg.static_ip.as_ref(), cfg.gateway.as_ref())?;

    validate_ssh(&cfg.ssh)?;
    validate_containers(&cfg.containers)?;

    validate_swap(cfg.swap.as_ref())?;
    validate_output_label(cfg.output_label.as_ref())?;
    validate_wallpaper(cfg.wallpaper.as_ref())?;
    validate_grub(&cfg.grub)?;
    validate_out_name(&cfg.out_name)?;

    validate_dnf(&cfg.dnf_repos, cfg.dnf_mirror.as_ref())?;
    validate_pacman(&cfg.pacman_repos, cfg.pacman_mirror.as_ref())?;

    validate_sha256(cfg.expected_sha256.as_ref())?;
    validate_swap_size(cfg.swap.as_ref())?;

    validate_encryption(
        cfg.encrypt,
        cfg.encrypt_passphrase.as_ref(),
        cfg.storage_layout.as_ref(),
    )?;

    validate_packages(&cfg.extra_packages)?;

    Ok(())
}
