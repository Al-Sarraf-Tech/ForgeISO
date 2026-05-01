# ForgeISO Security Contract

This document describes the build-artifact provenance, tamper-evidence, and
signature-verification model that ForgeISO releases ship under, and how an
end user verifies a downloaded release.

For the input-validation, supply-chain, and host-trust posture of the running
tool itself, see also [`security.md`](security.md).

For the architectural rationale (why we picked this model, what we rejected,
and how it maps to the S+ Security rubric), see
[ADR 0010](adr/0010-security-contract-desktop-tool.md).

## Why not SLSA

The S+ tier rubric points at SLSA Level 2/3 build provenance as the
service-shape standard. ForgeISO **does not implement SLSA** because the
project's CLAUDE.md absolute rules explicitly ban
`actions/attest-build-provenance` (the canonical SLSA tooling). The ban is
an organisational policy about GitHub Actions paid features and public-repo
gating; it overrides the rubric.

The underlying engineering need that SLSA addresses — provenance, tamper
evidence, verifiable signing identity, and a public transparency log — is
real. ForgeISO satisfies it with a different toolset.

## What we use instead

Every binary, package, checksum file, and SBOM in a ForgeISO release is
signed using **cosign** in keyless OIDC mode.

| Concern | SLSA mechanism (banned) | ForgeISO mechanism |
|---|---|---|
| Build provenance | `actions/attest-build-provenance` | cosign sign-blob from a release-tag workflow on the trusted repo |
| Signing identity | SLSA provenance attestation | Sigstore Fulcio cert tied to GitHub Actions OIDC subject |
| Tamper evidence | in-toto attestation | detached `.sig` + `.pem` per artifact |
| Public audit trail | n/a | Rekor transparency-log entry per signature |
| SBOM signing | SBOM-as-attestation | The syft-generated SBOM is treated as a release artifact and signed by the same flow |

The cosign signing happens in a dedicated `release-sign` CI job that runs
after the existing `release` job. The job needs `id-token: write` permission
to obtain the GitHub Actions OIDC token; cosign trades that token at Fulcio
for a short-lived signing certificate, signs each artifact, and writes a
Rekor transparency-log entry.

The signing key never exists. The certificate's lifetime is roughly ten
minutes, scoped to the workflow run that issued it. The Rekor entry is the
permanent record.

## Verifying a downloaded release

End users verifying a release should:

1. Install cosign (>= 2.0). See the upstream
   [installation guide](https://docs.sigstore.dev/cosign/installation/).
2. Download the release artifacts plus the `signatures/` directory from the
   GitHub Release page.
3. Run the verification script in this repo, or invoke cosign directly:

```bash
# Using the script in this repo (recommended):
REL_DIR=release-assets/ scripts/verify-release.sh

# Or, manually for a single artifact:
cosign verify-blob \
  --certificate signatures/forgeiso-1.0.0-linux-x86_64.tar.gz.pem \
  --signature   signatures/forgeiso-1.0.0-linux-x86_64.tar.gz.sig \
  --certificate-identity-regexp '^https://github\.com/Al-Sarraf-Tech/ForgeISO/\.github/workflows/.+@refs/tags/v.+$' \
  --certificate-oidc-issuer-regexp '^https://token\.actions\.githubusercontent\.com$' \
  forgeiso-1.0.0-linux-x86_64.tar.gz
```

A successful verification proves three things:

- the artifact bytes are exactly what was signed (tamper-evident),
- the signing identity matches the expected GitHub Actions OIDC subject
  (i.e. the release came from the `Al-Sarraf-Tech/ForgeISO` workflow on a
  release tag, not from a fork or a hand-built archive),
- a Rekor transparency-log entry exists for the signature (so a future
  rogue signing event would be publicly visible and auditable).

If verification fails for any artifact, **do not install it.**

## SBOM verification

Each release ships a CycloneDX-1.5 SBOM produced by syft, listed alongside
the binary artifacts. The SBOM is signed by the same cosign flow as every
other artifact, so the same `verify-blob` invocation works against it. This
chains the dependency manifest to the same Sigstore identity as the binary
itself: an attacker who tampered with either the binary or the SBOM would
break the corresponding signature.

## Reporting a security issue

If you discover a security issue, open a GitHub issue or contact the
maintainers directly. See [`security.md`](security.md) for the running-tool
security posture (input validation, supply chain, dependency policy).
