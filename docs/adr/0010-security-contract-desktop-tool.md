# ADR 0010: Security contract for a desktop build tool

- **Status**: Accepted
- **Date**: 2026-05-01

## Context

The S+ tier rubric (`~/.claude/TIER_RUBRIC.md`) defines Security progression as:

- **A**: SAST + DAST + SCA + secrets scanning + dependency pinning + signed releases + threat model
- **A+**: A + signed commits + SLSA L2 build provenance + automated supply-chain alerts
- **S**: A+ + SLSA L3 + reproducible builds + sigstore + WAF + zero secrets in code/CI
- **S+**: S + formal verification of critical security paths + insider-threat protections + cryptographic agility

ForgeISO already satisfies most of A and A+ in CI today: cargo-audit + cargo-deny
(SCA), gitleaks (secrets), Trivy fs scan (SAST + dependency CVEs), syft SBOMs,
pinned container images and Cargo.lock-locked builds, SHA-256 verification of
both source and output ISOs.

The rubric step from A+ → S explicitly names **SLSA L3** as the build-provenance
mechanism. ForgeISO **cannot adopt SLSA**: the project's CLAUDE.md absolute rule
bans `actions/attest-build-provenance` (the canonical SLSA tooling). The ban
is an organisational policy decision about GitHub Actions paid features and
public-repo gating; it overrides the rubric for this project.

But the underlying engineering need does still apply. A user installing
ForgeISO from a release archive deserves to know:

- the bytes haven't been tampered with in transit or at rest on the mirror,
- the artifact came from our build pipeline, not a fork or a hand-built tarball,
- there is a public, append-only audit trail of every signing event (so a
  compromised maintainer signing a malicious release would leave a public
  trace).

These three properties are what SLSA L2/L3 + Sigstore deliver in service-shape
deployments. ForgeISO can deliver the same properties using a strict subset of
the Sigstore stack — without invoking the banned attestation action.

| S/S+ Security rubric criterion | Service shape | ForgeISO desktop equivalent |
|---|---|---|
| SAST | SonarQube / CodeQL on every PR | clippy `-D warnings` + Trivy fs HIGH/CRITICAL gate (CI) |
| DAST | OWASP ZAP scan against running service | N/A — desktop binary has no exposed surface; substitute is `engine/tests/chaos.rs` fault-injection |
| SCA | Snyk / Dependabot | cargo-audit + cargo-deny + Trivy fs (CI) |
| Secrets scanning | gitleaks / trufflehog | gitleaks (CI security job, `--exit-code=1`) |
| Dependency pinning | renovate + lockfile commit | `Cargo.lock` committed; CI containers pin `rust:1.93-bookworm`; security-tool versions pinned via `ARG` |
| Signed releases | GPG signed tarballs | cosign sign-blob keyless (this ADR) |
| Threat model | STRIDE doc | `docs/runtime-security.md` (running-tool) + this ADR (release-pipeline) + ADR 0008 (reliability) |
| Signed commits | gpg/ssh-sign | branch-protection requires PR review on Al-Sarraf-Tech repo (CLAUDE.md absolute) |
| SLSA L2 build provenance | `actions/attest-build-provenance` (BANNED) | cosign sign-blob keyless OIDC + Rekor transparency-log entry per artifact |
| Automated supply-chain alerts | dependabot security alerts | RUSTSEC ignores in `deny.toml` carry rationale; Trivy fs on every PR |
| SLSA L3 | hermetic + non-falsifiable provenance | cosign keyless ties signing identity to GitHub OIDC workflow subject; Rekor is non-falsifiable |
| Reproducible builds | bit-identical rebuild | partial — `Cargo.lock` + pinned compiler; not yet verified bit-identical (TODO follow-up) |
| Sigstore | sigstore / cosign | cosign sign-blob (this ADR) |
| WAF | CloudFlare / AWS WAF | N/A — no public service to protect; substitute is input validation in `InjectConfig::validate()` (`docs/runtime-security.md`) |
| Zero secrets in code/CI | OIDC + workload identity everywhere | Keyless cosign uses GitHub Actions OIDC; no signing key stored as a secret |
| Formal verification | TLA+ / Lean specs | partial — property tests + golden contract test for public API; no formal proof |
| Insider-threat protections | code review + privileged access reviews | branch protection on Al-Sarraf-Tech repos (absolute); release-tag job gated on `startsWith(github.ref, 'refs/tags/')` |
| Cryptographic agility | algorithm-rotation runbook | cosign uses ECDSA-P256 by default; Sigstore upgrade path is upstream-managed |

## Decision

ForgeISO's release security contract is the desktop-equivalent of S/S+ rubric
Security criteria, codified as four guarantees:

### 1. **Every release artifact is signed**

For every binary, package (`.deb`, `.rpm`, `.pkg.tar.zst`, `.tar.gz`),
checksums file, and SBOM in `release-assets/`, a detached cosign signature
plus signing certificate is written to `release-assets/signatures/`.

Implementation: `scripts/sign-release.sh`. CI: a `release-sign` job runs
after the existing `release` job (orchestrator config; see
`docs/CI-INTEGRATION.md`).

### 2. **Signing identity is tied to a GitHub Actions OIDC subject**

The cosign keyless flow trades a GitHub Actions OIDC token at Fulcio for a
short-lived signing certificate. The certificate's identity field embeds the
workflow path and ref (`refs/tags/v*`). No signing key exists, so no signing
key can be stolen. Verification (script or manual) pins the expected
identity regexp to our repo and the expected issuer regexp to GitHub.

### 3. **Every signing event is logged in Rekor**

Cosign keyless signatures are recorded in the public Sigstore transparency
log (Rekor) by default. Anyone — including end users, downstream
distributors, security researchers — can audit the log to confirm a
specific release was signed by the expected workflow on the expected tag.
A rogue signing event would leave a public trace.

### 4. **The SBOM is signed by the same flow**

The syft-generated CycloneDX SBOM is treated as a first-class release
artifact and signed alongside the binaries. Tampering with either the
binary or its SBOM would break the corresponding signature, so dependency
provenance chains to the same Sigstore identity as code provenance.

## Test coverage for the contract

- `scripts/verify-release.sh` exercises the verify-blob path locally (no
  CI gate today; TODO: add a smoke test that signs a fixture file and
  verifies the round-trip in CI without contacting Fulcio).
- The CI `release-sign` job re-runs `verify-release.sh` on its own output
  before publishing, so a broken signature pipeline fails the release
  before any artifact is announced.

## Alternatives considered

- **in-toto / witness**: a more flexible attestation framework; would let
  us model multi-step pipelines (build → test → sign → publish) as a single
  attestation graph. Rejected for now: heavier integration than cosign
  sign-blob warrants for a 4-crate desktop tool with one release pipeline.
  Revisit if the pipeline grows multiple stages with separate trust
  boundaries.
- **slsa-github-generator**: provides SLSA L3 provenance without
  `actions/attest-build-provenance`. Rejected as also-banned under an
  absolute interpretation of the CLAUDE.md rule (it is the same family
  of GitHub-paid attestation tooling, generating attestations from a
  reusable workflow).
- **GPG signed tarballs**: traditional, but creates a key-management
  burden (revocation, rotation, web-of-trust) and provides no public
  transparency log. Cosign keyless eliminates the key and adds Rekor.
- **No release signing** (cap Security at A+ on the rubric): the
  cosign-based approach is small and well-supported; not signing would
  leave a known gap that the rubric calls out specifically.
- **Reproducible-builds verification** as the primary tamper-evidence:
  worth doing eventually as a complement, but it doesn't establish *who*
  built the artifact, only that any builder following the same recipe
  would get the same bytes. Listed as a TODO in the table above.

## Consequences

- **Positive**: clear contract for release-artifact provenance. Any user
  can verify with one script, no key handling.
- **Positive**: no signing key exists to steal, lose, rotate, or revoke.
- **Positive**: Rekor entries provide a public, third-party-hosted audit
  trail. Independent of GitHub.
- **Positive**: Security dimension of the tier rubric lifts A → toward S+
  (gated on `release-sign` job landing in orchestrator config and on the
  reproducible-builds follow-up).
- **Negative**: signing requires Sigstore Fulcio + Rekor reachability from
  the CI runner at release time. If Sigstore is down, releases pause.
  Acceptable risk; Sigstore SLA is publicly tracked and outages are rare.
- **Negative**: verification requires cosign on the user's machine. We
  document the install link in `docs/SECURITY.md`; checksums + manual
  visual inspection remain available as a fallback for pre-cosign users.
- **Process consequence**: any new artifact type added to a release (e.g.
  if we ship a Snap or an AppImage in the future) must be added to the
  `sign-release.sh` artifact list; `verify-release.sh` covers it
  automatically because it iterates over signatures it finds.

## Implementation reference

- `scripts/sign-release.sh` — signing entry point (operator + CI)
- `scripts/verify-release.sh` — verification entry point (consumer + CI)
- `docs/SECURITY.md` — end-user-facing description of the security model
- `docs/runtime-security.md` — running-tool security (input validation, supply chain)
- `docs/CI-INTEGRATION.md` — describes the `release-sign` CI job to add
- `docs/COMPLIANCE.md` — cross-references SC-12 / CIS 2 / CC8.1 to this ADR
- ADR 0008 (reliability contract) — the model this ADR mirrors structurally
