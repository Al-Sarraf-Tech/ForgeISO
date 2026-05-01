//! Tests for package source fields (apt / dnf / pacman repos and mirrors).
//!
//! Bodies preserved verbatim from the original `inject.rs` test module.

use super::super::*;

#[test]
fn inject_rejects_shell_metachar_in_apt_repo() {
    let cfg = InjectConfig {
        apt_repos: vec!["ppa:user/repo'; echo pwned".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_shell_metachar_in_apt_mirror() {
    let cfg = InjectConfig {
        apt_mirror: Some("http://mirror.example.com$(id)".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_accepts_valid_apt_repos() {
    let cfg = InjectConfig {
        apt_repos: vec!["deb http://archive.ubuntu.com/ubuntu noble main".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn inject_rejects_bare_url_as_apt_repo() {
    // A raw URL is not a valid sources.list line (missing "deb " prefix).
    let cfg = InjectConfig {
        apt_repos: vec!["http://archive.ubuntu.com/ubuntu".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "bare URL without 'deb ' prefix must be rejected"
    );
}

#[test]
fn inject_accepts_ppa_shorthand_as_apt_repo() {
    // PPA shorthands are handled via add-apt-repository in generated late-commands.
    let cfg = InjectConfig {
        apt_repos: vec!["ppa:deadsnakes/ppa".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "ppa: shorthand must be accepted (handled via add-apt-repository)"
    );
}

#[test]
fn inject_accepts_deb_src_apt_repo() {
    let cfg = InjectConfig {
        apt_repos: vec!["deb-src http://archive.ubuntu.com/ubuntu noble main".into()],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "deb-src line must be accepted");
}

#[test]
fn inject_rejects_apt_repo_without_deb_prefix() {
    // Arbitrary text that is not a valid sources.list line must be caught.
    let cfg = InjectConfig {
        apt_repos: vec!["http://ppa.launchpad.net/user/ppa/ubuntu".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "apt_repo missing 'deb ' prefix must be rejected"
    );
}

#[test]
fn inject_accepts_valid_deb_src_apt_repo() {
    let cfg = InjectConfig {
        apt_repos: vec![
            "deb http://archive.ubuntu.com/ubuntu noble main".into(),
            "deb-src http://archive.ubuntu.com/ubuntu noble main".into(),
        ],
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "valid deb/deb-src lines must pass");
}

#[test]
fn inject_rejects_apt_mirror_with_shell_metachar() {
    let cfg = InjectConfig {
        apt_mirror: Some("http://mirror.example.com/ubuntu; malicious".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn inject_rejects_dnf_mirror_with_sed_delimiter() {
    let cfg = InjectConfig {
        dnf_mirror: Some("https://mirror.example.com|evil".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_mirror with | (sed delimiter) must be rejected"
    );
}

#[test]
fn inject_accepts_valid_dnf_mirror() {
    let cfg = InjectConfig {
        dnf_mirror: Some("https://mirror.example.com/fedora".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "clean dnf_mirror URL must pass");
}

#[test]
fn inject_rejects_dnf_repo_url_with_single_quote() {
    let cfg = InjectConfig {
        dnf_repos: vec!["https://evil.example.com/'; rm -rf /".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_repo URL with single quote must be rejected"
    );
}

#[test]
fn inject_accepts_dnf_repo_stanza_with_dollar_sign() {
    // $releasever and $basearch are standard DNF stanza variables.
    // They go through a heredoc (not single-quoted shell), so $ is safe.
    let cfg = InjectConfig {
        dnf_repos: vec!["[rpmfusion-free]\nbaseurl=https://mirrors.rpmfusion.org/free/fedora/$releasever/$basearch\nenabled=1".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "dnf_repo stanza with $releasever must be accepted"
    );
}

#[test]
fn inject_rejects_pacman_mirror_with_single_quote() {
    let cfg = InjectConfig {
        pacman_mirror: Some("https://mirror.example.com/arch'; evil".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "pacman_mirror with single quote must be rejected (breaks shell quoting)"
    );
}

#[test]
fn inject_accepts_pacman_mirror_with_dollar_sign() {
    // Pacman mirror URLs are single-quoted in shell; $ is literal in single-quoted strings.
    let cfg = InjectConfig {
        pacman_mirror: Some("https://mirror.pkgbuild.com".into()),
        ..Default::default()
    };
    assert!(cfg.validate().is_ok(), "clean pacman_mirror URL must pass");
}

#[test]
fn inject_rejects_pacman_repo_with_newline() {
    let cfg = InjectConfig {
        pacman_repos: vec!["Server = https://good.mirror.com\nrm -rf /".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "pacman_repo with newline must be rejected (would break echo command)"
    );
}

#[test]
fn inject_accepts_valid_pacman_repo_entry() {
    // $repo and $arch are pacman template variables -- safe in single-quoted strings.
    let cfg = InjectConfig {
        pacman_repos: vec!["Server = https://mirror.pkgbuild.com/$repo/os/$arch".into()],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "standard pacman Server= line with template vars must be accepted"
    );
}

#[test]
fn inject_rejects_dnf_repo_stanza_containing_heredoc_sentinel() {
    // A line that is exactly the heredoc sentinel would terminate the
    // `cat > .repo << 'FORGEISO_REPO_EOF'` heredoc early, producing a
    // truncated .repo file.
    let cfg = InjectConfig {
        dnf_repos: vec![
            "[myrepo]\nbaseurl=https://mirror.example.com\nFORGEISO_REPO_EOF\ngpgcheck=1".into(),
        ],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_repo stanza containing heredoc sentinel line must be rejected"
    );
}

#[test]
fn inject_accepts_dnf_repo_stanza_with_sentinel_as_substring() {
    // The sentinel only terminates if it appears alone on a line -- as a
    // substring of a longer line it is harmless.
    let cfg = InjectConfig {
        dnf_repos: vec![
            "[myrepo]\n# generated by FORGEISO_REPO_EOF_marker\nbaseurl=https://mirror.example.com\n".into(),
        ],
        ..Default::default()
    };
    assert!(
        cfg.validate().is_ok(),
        "sentinel as substring of a longer line must be accepted"
    );
}

#[test]
fn inject_rejects_dnf_mirror_with_null_byte() {
    let cfg = InjectConfig {
        dnf_mirror: Some("https://mirror.example.com/\0evil".into()),
        ..Default::default()
    };
    assert!(
        cfg.validate().is_err(),
        "dnf_mirror with null byte must be rejected"
    );
}
