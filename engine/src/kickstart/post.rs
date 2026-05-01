use crate::autoinstall::build_feature_late_commands;
use crate::config::InjectConfig;
use crate::error::EngineResult;

/// Build and append the Kickstart `%post` block.
///
/// The block contains DNF mirror/repo/Docker setup followed by the rewritten
/// late-commands.  Engine-generated late-commands have any leading
/// `chroot /target ` stripped and `/target/` path prefixes rewritten to `/`
/// because `%post` already runs inside the installed-system chroot.
/// User-supplied trailing entries (`run_commands`, `extra_late_commands`)
/// pass through verbatim.
pub(super) fn append_post(cfg: &InjectConfig, lines: &mut Vec<String>) -> EngineResult<()> {
    let late_cmds = build_feature_late_commands(cfg)?;

    // Build DNF repo and mirror commands to prepend to %post
    let dnf_post = build_dnf_setup(cfg);

    // The last N commands from build_feature_late_commands are user-provided
    // (run_commands, extra_late_commands) and must pass through unchanged so
    // users can control their own paths (e.g. echo /data/target/file).
    // All preceding commands are engine-generated and MUST have /target/ paths
    // rewritten because %post runs inside the installed-system chroot where
    // there is no /target directory.
    let user_cmd_count = cfg.run_commands.len() + cfg.extra_late_commands.len();
    let generated_count = late_cmds.len().saturating_sub(user_cmd_count);

    let all_post: Vec<String> = dnf_post
        .into_iter()
        .chain(late_cmds.iter().enumerate().map(|(idx, cmd)| {
            if idx < generated_count {
                rewrite_chroot_path(cmd)
            } else {
                // User-provided (run_commands / extra_late_commands):
                // pass through unchanged.
                cmd.clone()
            }
        }))
        .collect();

    if !all_post.is_empty() {
        lines.push(String::new());
        lines.push("%post".to_string());
        for cmd in &all_post {
            lines.push(cmd.clone());
        }
        lines.push("%end".to_string());
    }

    Ok(())
}

/// Engine-generated commands target the cloud-init install context where the
/// system root is mounted at `/target`.  Inside the Kickstart `%post` chroot
/// the same paths must resolve under `/`.  The `chroot /target ` prefix form
/// is stripped, and bare `/target/` path prefixes are collapsed.
fn rewrite_chroot_path(cmd: &str) -> String {
    if let Some(inner) = cmd.strip_prefix("chroot /target ") {
        inner.replace("/target/", "/").replace("/target ", "/ ")
    } else {
        cmd.replace("/target/", "/")
    }
}

/// Build DNF mirror, container, and extra-repo commands for the head of
/// `%post`.  All commands are emitted with `2>/dev/null || true` guards so
/// missing repo files do not fail the whole install.
fn build_dnf_setup(cfg: &InjectConfig) -> Vec<String> {
    let mut dnf_post: Vec<String> = Vec::new();

    // Override primary mirror if requested
    if let Some(mirror) = &cfg.dnf_mirror {
        // Write a custom .repo file that overrides the fedora + updates baseurl
        dnf_post.push(format!(
            r"sed -i 's|^baseurl=.*|baseurl={mirror}/$releasever/Everything/$basearch/os/|' /etc/yum.repos.d/fedora.repo 2>/dev/null || true"
        ));
        dnf_post.push(format!(
            r"sed -i 's|^baseurl=.*|baseurl={mirror}/$releasever/Everything/$basearch/os/|' /etc/yum.repos.d/fedora-updates.repo 2>/dev/null || true"
        ));
    }

    // Docker CE — uses the upstream Docker Inc. Fedora repository.
    if cfg.containers.docker {
        dnf_post.push("dnf config-manager --add-repo https://download.docker.com/linux/fedora/docker-ce.repo 2>/dev/null || true".to_string());
        dnf_post.push("dnf install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin 2>/dev/null || true".to_string());
        dnf_post.push("systemctl enable docker".to_string());
        for user in &cfg.containers.docker_users {
            dnf_post.push(format!("usermod -aG docker {user}"));
        }
    }

    // Podman — available from Fedora's standard repos (no extra repo needed).
    if cfg.containers.podman {
        dnf_post.push("dnf install -y podman 2>/dev/null || true".to_string());
    }

    // Install extra repos: URL entries → dnf install; stanza entries → write .repo file
    for (idx, repo) in cfg.dnf_repos.iter().enumerate() {
        let trimmed = repo.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            // Plain URL — install the RPM or .repo file directly
            if trimmed.to_ascii_lowercase().ends_with(".rpm") {
                dnf_post.push(format!("dnf install -y '{trimmed}' 2>/dev/null || true"));
            } else {
                // Assume it's a .repo URL
                dnf_post.push(format!(
                    "dnf config-manager --add-repo '{trimmed}' 2>/dev/null || true"
                ));
            }
        } else {
            // Treat as a verbatim .repo stanza. Use a heredoc so multi-line
            // stanza content is written with real newlines — {stanza:?} would
            // use Debug formatting which escapes '\n' as two-char sequences,
            // producing a single-line .repo file that dnf cannot parse.
            let repo_name = format!("forgeiso-extra-{idx}");
            dnf_post.push(format!(
                "cat > /etc/yum.repos.d/{repo_name}.repo << 'FORGEISO_REPO_EOF'\n{stanza}\nFORGEISO_REPO_EOF",
                repo_name = repo_name,
                stanza = trimmed,
            ));
        }
    }

    dnf_post
}
