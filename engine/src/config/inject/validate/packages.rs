//! Validators for package manager (apt / dnf / pacman) repos and mirrors.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::error::{EngineError, EngineResult};

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
