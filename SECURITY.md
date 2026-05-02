# Security Policy

ForgeISO takes security seriously. This file is the entry point for the
GitHub security advisory dispatcher; the full security model lives in
[`docs/SECURITY.md`](docs/SECURITY.md) (release-artifact provenance) and
[`docs/runtime-security.md`](docs/runtime-security.md) (input validation,
supply chain, host trust posture).

## Reporting a vulnerability

Please **do not** open public GitHub issues for security vulnerabilities.

Use one of the following private channels:

1. **Preferred — GitHub Security Advisory:** open a private advisory at
   https://github.com/Al-Sarraf-Tech/ForgeISO/security/advisories/new.
2. **Email:** contact the maintainer at the address listed in the
   project's `Cargo.toml` `authors` field.

Include:

- A description of the issue and the affected version.
- Reproduction steps (a minimal command-line invocation if possible).
- The output of `forgeiso --version` and `uname -a` from a host where
  the issue reproduces.
- Your assessment of severity / blast radius if known.

## Disclosure timeline

- **Acknowledgement:** within 5 business days.
- **Initial triage and severity assessment:** within 14 days.
- **Patch + coordinated disclosure:** target 90 days from triage,
  shorter for actively exploited issues.
- **Public advisory:** published after the fix ships in a tagged
  release. Reporter is credited unless they request otherwise.

## Supported versions

ForgeISO follows [SemVer](https://semver.org/). During the 0.x series,
only the latest minor version receives security fixes. Once 1.0 ships,
the most recent two minor versions of the latest major release will be
supported, plus the previous major release for 6 months after a new
major ships.

| Version | Supported |
|---------|-----------|
| 0.3.x   | yes (current) |
| <0.3    | no  |

## Verifying release artifacts

ForgeISO release binaries are signed with cosign keyless signing. See
[`docs/SECURITY.md` "Verification"](docs/SECURITY.md) for the exact
`cosign verify-blob` invocation and the GitHub Actions workflow subject
expected for each release.
