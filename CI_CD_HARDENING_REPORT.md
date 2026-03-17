# CI/CD Hardening Report — ForgeISO

**Date:** 2026-03-14
**Branch:** `ci/assurance-hardening`
**Baseline commit:** `ece9003` (main)

---

## Changes Made

### 1. Shell Script Linting (NEW)

**Gap identified:** 28 shell scripts in `scripts/` and `.github/scripts/` had
no automated linting. Shell scripts drive the entire CI pipeline, release
packaging, and matrix testing infrastructure.

**Resolution:** Added a `shell-lint` job to `.github/workflows/ci.yml` that
runs in parallel with the existing 7-stage Docker CI matrix:

- **shellcheck** (`--severity=warning --shell=bash`) — static analysis for
  common shell scripting errors, quoting issues, and portability problems.
- **shfmt** (`-d -i 2 -ci -bn`) — enforces consistent formatting (2-space
  indent, case indent, binary operators at start of next line).

The job runs directly on `ubuntu-latest` without Docker (lightweight, fast).
It covers all `.sh` files under `scripts/` and `.github/scripts/`.

### 2. Concurrency Controls (NEW)

**Gap identified:** No concurrency group was defined. Redundant CI runs on the
same branch (e.g., rapid pushes during development) would all run to completion,
wasting GitHub Actions minutes.

**Resolution:** Added a `concurrency` block to `ci.yml`:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.head_ref || github.ref }}
  cancel-in-progress: true
```

This cancels in-progress runs when a new push arrives on the same branch.
Tag-triggered release runs use `github.ref` (unique per tag) so they are never
cancelled by this rule.

### 3. Assurance Documentation (NEW)

**Created:** `ASSURANCE.md` — a single document describing:

- The 7-stage parallel Docker CI pipeline architecture
- Security scanning tools and their roles
- SBOM generation (CycloneDX + SPDX)
- Test coverage (658+ tests)
- Pre-push hook enforcement
- License compliance via `deny.toml`
- Local CI execution instructions

---

## Pre-Existing Controls (Verified)

| Control | Status | Notes |
|---|---|---|
| 7-stage Docker CI (C1-C7) | Operational | All stages run in parallel ephemeral containers |
| cargo-deny (license + advisory) | Operational | Enforced in C2 via `deny.toml` |
| cargo-audit | Operational | RustSec advisory checks in C2 |
| trivy (container scanning) | Operational | Image vulnerability scanning in C4 |
| syft (SBOM) | Operational | CycloneDX generation in C2; both formats at release |
| grype (vulnerability) | Operational | Scans against SBOM output in C4 |
| gitleaks (secret detection) | Operational | Repository history scanning in C4 |
| Pre-push hook | Operational | Runs full CI locally before push |
| Ephemeral containers | Operational | `--rm` + volume cleanup after every stage |
| Per-stage cache isolation | Operational | Cache keys include matrix.id |
| GITHUB_TOKEN least privilege | Operational | Default read-only; write only for release |
| fail-fast: false | Operational | Full report on every run |

---

## Remaining Recommendations

1. **Pin GitHub Actions versions to SHA hashes** — currently using `@v4` tags
   for `actions/checkout`, `actions/cache`, `actions/upload-artifact`. Pinning
   to full SHAs reduces supply-chain risk.

2. **Add CODEOWNERS** — enforce required reviewers for CI configuration changes
   (`.github/`, `containers/`, `scripts/ci/`).

3. **Dependabot for Actions** — enable Dependabot to automatically propose
   updates to GitHub Actions dependencies.

4. **shellcheck/shfmt baseline** — the initial run may surface warnings in
   existing scripts. Address these incrementally; the CI job will enforce the
   standard going forward.

---

## Files Modified

| File | Change |
|---|---|
| `.github/workflows/ci.yml` | Added concurrency controls; added shell-lint job |
| `ASSURANCE.md` | New — pipeline and security controls documentation |
| `CI_CD_HARDENING_REPORT.md` | New — this report |
