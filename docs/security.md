# Security Notes

## Local-first execution

ForgeISO performs product workflows locally on the host machine. It does not open product-side network services or require a remote agent.

## Host trust boundary

- Inspect the source ISO before building
- Use only local overlay directories you trust
- Review generated reports before distributing a remastered image
- Keep local toolchain packages current on the Linux host

## Input validation

All user-supplied values that flow into shell commands or cloud-init YAML pass through `InjectConfig::validate()` before any generation occurs. The validation gate covers:

| Field(s) | Validation | Reason |
|---|---|---|
| `username`, `hostname`, `locale`, `keyboard_layout`, `timezone` | `is_safe_identifier` — alphanumeric, `-`, `_`, `.`, `:` only | Embedded in shell commands and YAML |
| `packages`, `user_groups`, `services_enable`, `services_disable` | `is_safe_identifier` | Used in `apt install`, `systemctl`, `usermod` |
| `firewall.allowed_tcp_ports`, `firewall.allowed_udp_ports` | `is_safe_identifier` | Used in `ufw allow` commands |
| `ssh_authorized_keys` | Must start with `ssh-` or `ecdsa-` | Written to `authorized_keys` |
| `dns_servers`, `ntp_servers` | `is_safe_identifier` | Used in cloud-init YAML and `printf` commands |
| `sudo_commands`, `apt_repos`, `mounts` | Block shell metacharacters (`;`, `&`, `\|`, `$`, `` ` ``, `'`, `"`, `\`, `\n`) | Written to sudoers / apt sources / fstab |
| `apt_mirror`, `proxy.http_proxy`, `proxy.https_proxy` | Block shell metacharacters | Used in sed replacements and env vars |
| `proxy.no_proxy` | Block shell metacharacters | Written to environment config |
| `grub.default_entry`, `grub.cmdline_extra` | Block shell metacharacters AND `/` (sed delimiter) | Interpolated into `sed s///` patterns |
| `expected_sha256` | 64-character hex string (GUI-side validation) | Compared against ISO hash |

The `is_safe_identifier` function rejects any character outside `[a-zA-Z0-9._:/-]`.

Late commands (`--late-command`) are intentionally **not** validated — they are an explicit escape hatch for arbitrary shell execution, documented as such in `--help`.

## Supply chain

### CI container images

- All Rust CI containers pin `rust:1.93-bookworm` (not floating `rust:1-bookworm`)
- Security scanner versions are pinned via `ARG` in Dockerfiles (trivy, syft, grype)
- Installer scripts are downloaded to a temp file and then executed (not piped via `curl | sh`)

### Dependency policy

`deny.toml` enforces:
- License allow-list (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0)
- Advisory database checks via `cargo-deny` and `cargo-audit` (CI stage C2)
- Known unmaintained crate suppressions are documented with rationale

### GitHub Actions

- Top-level workflow `permissions: contents: read` restricts the default `GITHUB_TOKEN`
- Only the `release` job escalates to `contents: write`
- All 7 CI stages must pass before the release job runs
- Ephemeral Docker volumes are destroyed after each stage

## CI/CD containers

CI containers are used only for pipeline execution. They are ephemeral and removed when the pipeline completes (`--rm` flag + explicit volume cleanup).

## Reporting vulnerabilities

If you discover a security issue, please open a GitHub issue or contact the maintainers directly.
