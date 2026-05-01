//! Validators for SSH authorized_keys and container runtimes.
//!
//! Logic preserved verbatim from the original monolithic
//! `engine/src/config/inject.rs::validate()`.

use crate::config::components::{ContainerConfig, SshConfig};
use crate::config::validation::is_safe_identifier;
use crate::error::{EngineError, EngineResult};

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
