# ForgeISO Runbooks

Operator playbook for diagnosing and recovering from ForgeISO failures.
Each entry maps a symptom (error code, log line, or stderr message) to
probable cause, diagnostic steps, and recovery actions.

Errors come from `engine/src/error.rs` (`EngineError` enum). The CLI
surfaces them prefixed with the variant name; the JSON tracing channel
(see `cli/src/obs.rs`) records the same content with structured fields.

Logs are written one JSON object per line to a daily-rolling file at
`<log_dir>/forgeiso.log.YYYY-MM-DD`. The default `log_dir` is
`$XDG_STATE_HOME/forgeiso` (or `~/.local/state/forgeiso/logs`); override
with `FORGEISO_LOG_DIR=/path/to/dir`. Filter with `jq`:

```bash
jq 'select(.level == "ERROR" or .level == "WARN")' \
  ~/.local/state/forgeiso/logs/forgeiso.log.$(date -I)
```

The TUI writes to `forgeiso-tui.log.<date>` and the GUI to
`forgeiso-gui.log.<date>` in the same directory.

For the engine-side guarantee that each error code below is actually
produced by the engine (rather than panicking, hanging, or silently
producing a corrupt artefact), see [`docs/CHAOS.md`](CHAOS.md) — the
chaos / fault-injection suite that pairs each runbook entry with an
`EngineError`-asserting test in `engine/tests/chaos.rs`.

---

## E001 — Source ISO Not Found

**Symptom**

```
not found: source ISO does not exist: /path/to/file.iso
```

**Cause** Path supplied to `--source` does not exist, or the cached download
was reaped/deleted between calls.

**Diagnose**

```bash
ls -la /path/to/file.iso
ls -la "$(forgeiso doctor --json | jq -r '.cache_root // empty')"
forgeiso inspect --source /path/to/file.iso
```

**Recovery**

- Verify the path. Use `--preset <name>` for a known good URL/path resolver.
- For a previously-downloaded ISO that has aged out of cache, simply re-run
  `forgeiso build|inject --source <url>` — the engine will re-download.
- For a manually-deleted file, restore from backup or re-download.

---

## E010 — Tooling Missing

**Symptom**

```
tooling missing: xorriso
tooling missing: unsquashfs
tooling missing: mksquashfs
tooling missing: qemu-system-x86_64
```

**Cause** A required external tool is not on `$PATH`. ForgeISO probes for
tools via `which::which()` and emits this variant when a hard dependency
is absent.

**Diagnose**

```bash
forgeiso doctor --json | jq '.tooling'
which xorriso unsquashfs mksquashfs qemu-system-x86_64
```

**Recovery** Install the missing tool with the host package manager:

| Tool                   | Fedora                            | Debian/Ubuntu        |
| ---------------------- | --------------------------------- | -------------------- |
| `xorriso`              | `dnf install xorriso`             | `apt install xorriso` |
| `unsquashfs`/`mksquashfs` | `dnf install squashfs-tools`   | `apt install squashfs-tools` |
| `qemu-system-x86_64`   | `dnf install qemu-system-x86`     | `apt install qemu-system-x86` |
| `OVMF` (UEFI firmware) | `dnf install edk2-ovmf`           | `apt install ovmf`   |
| `trivy`/`syft`/`grype` | follow upstream install scripts   | follow upstream install scripts |

`forgeiso doctor` enumerates every probed tool and reports which optional
features (Mint preseed, Arch archinstall, UEFI test, scan suite) become
available once each is installed.

---

## E020 — squashfs-tools Missing (rootfs repack)

**Symptom**

```
runtime error: failed to repack squashfs: unsquashfs not found
runtime error: failed to repack squashfs: mksquashfs failed (exit code 1)
```

**Cause** The Ubuntu / Mint / Arch live rootfs path needs both `unsquashfs`
(extract) and `mksquashfs` (repack). Either tool missing or the repack
exited non-zero (usually disk full or perms).

**Diagnose**

```bash
which unsquashfs mksquashfs
df -h /tmp $TMPDIR ${FORGEISO_WORKSPACE_BASE:-/tmp}
ls -la <workspace>/work/squashfs-extract/
```

**Recovery**

- Install: `dnf install squashfs-tools` (Fedora) or `apt install squashfs-tools`
  (Debian/Ubuntu).
- Free disk space — repack typically needs 2-3x the rootfs size as scratch.
- If permissions failed, ensure `$FORGEISO_WORKSPACE_BASE` is owned by
  the invoking user; do not run ForgeISO with `sudo` against a workspace
  rooted in a privileged path.

---

## E030 — xorriso Missing or Failing

**Symptom**

```
tooling missing: xorriso
runtime error: xorriso failed (exit code N)
```

**Cause** xorriso is the canonical ISO mount/repack tool. Without it,
ForgeISO cannot read files from inside an ISO (`/.disk/info`, `/.treeinfo`,
boot reports) or repack a customized ISO.

**Diagnose**

```bash
which xorriso
xorriso -version
forgeiso inspect --source /path/to/file.iso --json | jq '.warnings'
```

**Recovery**

- Install: `dnf install xorriso` or `apt install xorriso`.
- Inspect mode degrades gracefully: when xorriso is absent, ForgeISO
  reads only the ISO-9660 primary volume label (sector 16) and warns.
- Inject/build modes hard-fail without xorriso. There is no fallback.

---

## E040 — Permission Denied on Output Directory

**Symptom**

```
io error: Permission denied (os error 13): /path/to/output
runtime error: failed to create workspace: Permission denied
```

**Cause** The `--out` directory or workspace base is not writable by the
invoking user. Common causes: directory owned by root from a prior `sudo`
run; SELinux denial; mounted filesystem is read-only.

**Diagnose**

```bash
ls -la /path/to/output
stat /path/to/output
mount | grep "$(stat -c '%m' /path/to/output)"
# SELinux:
sestatus
ls -lZ /path/to/output
journalctl --since "5 min ago" | grep AVC
```

**Recovery**

- Pick a user-writable directory: `--out ~/forgeiso-output` or `--out /tmp/forgeiso`.
- Fix ownership: `sudo chown -R $USER:$USER /path/to/output` (NOT recommended
  for system paths — choose a different `--out` instead).
- Remount the volume read-write if needed.
- For SELinux denials, label the directory: `restorecon -Rv /path/to/output`.

ForgeISO **never** runs as root. Never `sudo forgeiso` — the resulting
artifacts will be unwritable for normal users and may include
root-owned cache files that fail subsequent runs.

---

## E050 — Filesystem Safety Violation

**Symptom**

```
filesystem safety violation: path traversal detected: ../../etc/passwd
filesystem safety violation: refusing to write outside workspace root
```

**Cause** A path supplied via `--overlay` or an autoinstall config field
escaped the workspace root via `..` or absolute path. ForgeISO uses
`workspace::safe_join()` to refuse symlink/path-escape attempts.

**Diagnose**

```bash
# Inspect the offending path:
realpath --no-symlinks <suspect-path>
# Compare against workspace root:
echo "$FORGEISO_WORKSPACE_BASE"
```

**Recovery** Sanitize the input. Overlay paths are interpreted relative
to the overlay root; do not pass `..` in any user-controlled field.
This is a security guard — bypass would expose the host filesystem to
malformed ISO content.

---

## E060 — Malformed Kickstart / Cloud-init / Preseed Config

**Symptom**

```
invalid config: hostname must match [a-z0-9-]+
invalid config: storage_layout must be one of: lvm|direct|zfs
invalid config: port must be 1..=65535
yaml error: invalid type: integer, expected a string at line 12
```

**Cause** Validation in `engine/src/config/validation.rs` rejected an
identifier, port, CIDR, or path before any IO occurred. This is a guard
against producing autoinstall files that the target installer would
silently ignore or, worse, accept dangerously.

**Diagnose**

- Re-read the field name in the error message and consult `forgeiso inject --help`
  for the documented format.
- For YAML supplied via `--autoinstall <file>`, validate with
  `python3 -c "import yaml,sys;yaml.safe_load(open(sys.argv[1]))" your.yaml`.

**Recovery** Fix the offending field. Common gotchas:

- `--hostname` must be a valid Linux hostname: `[a-z0-9][a-z0-9-]*` and
  no longer than 63 chars per label.
- `--allow-port` / `--deny-port` accept `<num>` or `<num>/proto` (e.g.
  `22/tcp`).
- `--static-ip` must be `<ip>/<prefix>` (CIDR), not `<ip> <netmask>`.
- `--ssh-key` must be a single SSH public key line (`ssh-ed25519 AAAA... user@host`).
- `--password-file` and `--ssh-key-file` must be readable; passwords are
  hashed in-process and never written to disk in plaintext.

---

## E070 — Distro Detection Failure

**Symptom**

```
inspect: distro=unknown release=unknown arch=unknown
runtime error: cannot inject: distro could not be detected from source ISO
```

**Cause** ForgeISO inspects `/.disk/info`, `/.treeinfo`, `/arch/version`,
and the primary volume label to identify the distro. None of those
matched any known pattern (Ubuntu, Mint, Fedora, Arch).

**Diagnose**

```bash
forgeiso inspect --source /path/to/file.iso --json | jq '{distro, release, volume_id, warnings}'
xorriso -indev /path/to/file.iso -ls /.disk/info /.treeinfo /arch/version 2>&1 | head
```

**Recovery**

- Pass `--distro <ubuntu|fedora|mint|arch>` explicitly to `forgeiso inject`
  to bypass detection.
- If the ISO is a derivative (e.g. Pop!_OS), it may identify as Ubuntu
  via `.disk/info`; pass `--distro ubuntu`.
- If the ISO is genuinely unsupported, `forgeiso inject` will refuse —
  ForgeISO targets the four supported distros and does not silently
  produce broken output for others.

---

## E080 — SHA-256 Mismatch on Source ISO

**Symptom**

```
runtime error: SHA-256 mismatch for /path/to.iso: expected <hex>, got <hex>
```

**Cause** The `--expected-sha256` flag was supplied and the actual hash
of the source ISO did not match. The operation aborted **before** any
modification to prevent operating on tampered or corrupt content.

**Diagnose**

```bash
sha256sum /path/to.iso
forgeiso verify --source /path/to.iso \
  --sums-url https://releases.ubuntu.com/24.04/SHA256SUMS
```

**Recovery**

- If the upstream hash is correct, your local ISO is corrupt — re-download.
- If you supplied the wrong expected hash, fix `--expected-sha256` to
  match the upstream SHA256SUMS line for your specific file.
- Never override SHA-256 verification on production builds; it is the
  only line of defence against tampered downloads.

---

## E090 — Network / Download Failure

**Symptom**

```
network error: download failed with status 404
network error: download failed after 3 attempts: https://...
http error: error sending request for url
```

**Cause** Source ISO URL returned non-2xx, network is unreachable, or
the download timed out. ForgeISO retries 3 times with exponential
backoff (1s, 2s) before giving up.

**Diagnose**

```bash
curl -I <url>
nslookup releases.ubuntu.com
ip route
```

**Recovery**

- 404: the upstream renamed/moved the file. Use `forgeiso sources resolve <preset>`
  to get the current canonical URL, or visit the distro's downloads page.
- DNS / no route: fix host networking; check `/etc/resolv.conf`.
- Behind a proxy: export `HTTPS_PROXY=https://...`. ForgeISO honors the
  standard `reqwest`/`hyper` proxy env vars.
- For air-gapped environments, download manually and pass `--source <local-path>`.

---

## E100 — Policy Violation (strict secrets scan)

**Symptom**

```
policy violation: Strict secrets policy failed
```

**Cause** The scanner detected one or more secret markers
(`BEGIN PRIVATE KEY`, `AKIA*`, `ghp_*`, `xoxb-*`, `token=`) inside the
target and `policy.strict_secrets = true` was set.

**Diagnose**

```bash
jq '.' <out_dir>/secrets.json
```

**Recovery**

- True positive: remove the leaked secret from the source content. **Rotate
  the secret upstream** (assume compromise once it has been on disk in
  shared infrastructure).
- False positive: file scanned was not a secret. Either move the file
  out of the scan target, or relax `strict_secrets` to `false` in the
  scan policy. Document why in your build manifest.

---

## Operational Scenarios (not E-codes)

### log_dir Unwritable Fallback

**Symptom**

```
forgeiso: log_dir /home/<user>/.local/state/forgeiso not writable, JSON logs disabled: <err>
```

**Cause** `mkdir -p` failed on the configured log directory (mounted
filesystem read-only, perm denied, path exists as a non-directory).

**Diagnose**

```bash
ls -la "$(dirname ~/.local/state/forgeiso)"
mount | grep "$(stat -c '%m' ~/.local/state)"
```

**Recovery**

- Fix the underlying perm/mount issue; or
- Repoint with `FORGEISO_LOG_DIR=/tmp/forgeiso-logs forgeiso ...`

ForgeISO keeps running — only the JSON log channel is disabled. The
existing stderr/stdout output, the in-app TUI log pane, and the GUI log
pane are unaffected. Tracing init is fail-open by design.

### Build Reports Missing After Successful Run

**Symptom** `forgeiso build` exited 0 but `report.html` / `report.json`
are absent or empty under `<workspace>/reports/`.

**Cause** Disk filled mid-write, or the workspace base was reaped by a
parallel `tmpwatch`/`systemd-tmpfiles` cycle while the build ran.

**Diagnose**

```bash
df -h "$FORGEISO_WORKSPACE_BASE"
journalctl --since "10 min ago" | grep -E 'tmp|workspace'
```

**Recovery** Re-run with an explicit workspace base on a non-tmp volume:

```bash
FORGEISO_WORKSPACE_BASE=/var/cache/forgeiso forgeiso build ...
```

### Trivy/Syft/Grype Reports Show Zero Findings

**Symptom** Scan completes but every tool reports zero severity counts.

**Cause** `severities` is inferred from substring matches on the tool's
JSON output; if a tool is genuinely clean **or** if a tool ran against
the wrong target (e.g. an empty workspace dir), zeros are returned.

**Diagnose**

```bash
ls -la <out_dir>/{trivy,syft,grype}.json
jq 'length' <out_dir>/trivy.json
forgeiso doctor --json | jq '.tooling | with_entries(select(.key | test("trivy|syft|grype")))'
```

**Recovery** Verify the scan target. If the target is correct and the
report is non-empty but counts are zero, the substring inference is too
permissive — file an issue. Counts are an indicator, not authoritative;
always inspect the raw tool JSON for production decisions.

### GUI Won't Start (`No graphical display detected`)

**Symptom**

```
No graphical display detected. Use `forgeiso-desktop` from a desktop session, or run `forgeiso-tui` / `forgeiso` on headless systems.
```

**Cause** Neither `$DISPLAY` nor `$WAYLAND_DISPLAY` is set. The GUI
refuses to spawn rather than crash on missing windowing.

**Recovery** Use `forgeiso-tui` on headless systems or run from a desktop
session. For SSH X11 forwarding, ensure `ssh -X` and `$DISPLAY` are set
on the remote.

---

## Engine Public-API Contract

The `forgeiso-engine` crate is consumed by `cli`, `tui`, `forge-slint`, and
external integrators. Drift in its public surface — adding, removing, or
changing the signature of any `pub` item — is a breaking change and must
be deliberated, not accidental.

### Mechanism

A goldenfile snapshot of every public item lives at
`engine/tests/public-api.golden`. It is produced by
[`cargo public-api`](https://github.com/cargo-public-api/cargo-public-api),
which emits a deterministic, line-per-item dump of the crate's API
surface (modules, types, fns, impls, blanket-impls).

The contract test `engine/tests/api_contract.rs::engine_public_api_matches_golden`
re-runs `cargo public-api -p forgeiso-engine` and diffs the result
against the golden. Any divergence fails the test with a summary of
added/removed lines.

### What triggers a failure

- A new `pub` item (struct, enum, fn, trait, type alias, module) reaches the API surface.
- An existing `pub` item is removed, renamed, made private, or relocated.
- A signature change: parameter list, generics, return type, lifetime bounds.
- A new derived trait (`Clone`, `Debug`, `Serialize`, etc.) on a `pub` type — adding/removing.
- A re-export change in `engine/src/lib.rs`.

The test is opt-in via `FORGEISO_RUN_API_CONTRACT=1` because it shells
out to nightly rustdoc; the CI gate sets the variable so PRs are
checked, while local devs without nightly are not blocked.

### How to update the golden

When the change is intentional:

1. Re-capture the golden:

   ```bash
   ./scripts/regenerate-api-golden.sh
   ```

   (Equivalent to `cargo public-api -p forgeiso-engine > engine/tests/public-api.golden`.)

2. Inspect the diff:

   ```bash
   git diff -- engine/tests/public-api.golden
   ```

3. Write an ADR under `docs/adr/NNNN-<short-title>.md` and link it from
   `docs/adr/README.md`. The ADR must cover:

   - **Context** — why the existing API surface was insufficient or wrong.
   - **Decision** — what changed (additions, removals, signature changes).
   - **Alternatives** — what was rejected and why.
   - **Consequences** — downstream impact on `cli`, `tui`, `forge-slint`,
     external consumers, MSRV, and semver (a removal/signature change is
     a major bump under semver; an addition is minor).

4. Stage the golden, the ADR, and the index update **in the same commit**:

   ```bash
   git add engine/tests/public-api.golden \
     docs/adr/NNNN-*.md docs/adr/README.md
   git commit
   ```

### When NOT to update the golden

- "It's just a one-line refactor" — if the surface changed, the consumers care.
- A blanket-impl shows up because a dependency added a new trait — investigate
  the dep upgrade first; spurious blanket impls usually mean the public surface
  leaks an internal type that should not be `pub`.
- The test fails on someone else's branch — they own the ADR; do not silently
  regenerate over their change.

### Local validation

```bash
cargo install cargo-public-api --locked   # one-time
rustup toolchain install nightly          # one-time, cargo-public-api needs rustdoc JSON
FORGEISO_RUN_API_CONTRACT=1 cargo test -p forgeiso-engine --test api_contract
```

---

## Health Check Procedure

Run after any change to host tooling, output paths, or the cache layer:

```bash
forgeiso doctor --json | jq '{linux_supported, tooling, distro_readiness, warnings}'
forgeiso inspect --source <local-iso> --json | jq '{distro, release, sha256, warnings}'
ls -la "${FORGEISO_LOG_DIR:-$HOME/.local/state/forgeiso}"
tail -5 "${FORGEISO_LOG_DIR:-$HOME/.local/state/forgeiso}"/forgeiso.log.$(date -I) 2>/dev/null | jq -c .
```

All four should produce non-empty, non-error output. If any fails, find
the matching E-code section above.

---

## Mutation Testing (engine quality gate)

`docs/MUTATION.md` documents the cargo-mutants setup that protects the
inject + autoinstall/ubuntu modules. Operator cheat-sheet:

```bash
scripts/run-mutants.sh                              # full run, enforce threshold
scripts/run-mutants.sh --check-only                 # fast compile-only smoke
scripts/run-mutants.sh --in-diff origin/main        # PR / pre-push gate
FORGEISO_MUTANTS_THRESHOLD=85 scripts/run-mutants.sh
```

Threshold is 80 % by default. Survivors that escape are triaged via the
recipe in `docs/MUTATION.md` (read mutant -> add unit test in the
matching `engine/src/config/inject/tests/<concern>.rs` -> re-run
`--in-diff`).
