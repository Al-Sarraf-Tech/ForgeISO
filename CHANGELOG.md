# Changelog

All notable changes to ForgeISO. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [SemVer](https://semver.org/spec/v2.0.0.html).
Release process: [docs/runbook-release.md](docs/runbook-release.md). Stability commitment: [STABILITY.md](STABILITY.md). Deprecation policy: [DEPRECATION.md](DEPRECATION.md).

## [Unreleased]

### Added
- **Tag-triggered release workflow** (1.0 readiness, Round 4) at `.github/workflows/release-build.yml`. Fires on push of any `v*.*.*` tag and produces a fully-signed GitHub Release: builds CLI/TUI/forge-slint binaries, runs `scripts/release/make-packages.sh` (RPM + DEB + pacman + tarball + checksums), generates CycloneDX + SPDX SBOMs via syft, signs every artifact through `scripts/sign-release.sh` (cosign keyless OIDC against Sigstore Fulcio with Rekor transparency log entries), runs `scripts/verify-release.sh` as a smoke test before publish, and uploads everything via `softprops/action-gh-release` (SHA-pinned). Per ADR 0010 the keyless flow is the desktop-tool equivalent of SLSA L2/L3 build provenance — no signing key exists to steal, identity is tied to the GitHub Actions OIDC subject, and every signing event is publicly auditable in Rekor. Honors all CLAUDE.md absolute rules (no `actions/attest-build-provenance`, self-hosted `[self-hosted, rust-slim]` runner, no AI co-author attribution). All workflow expressions that originate outside the file are routed through `env:` to defuse the GitHub Actions injection class.

- **Real-LTS integration test** (1.0 readiness, Round 3). New `engine/tests/real_lts_integration.rs` exercises `ForgeIsoEngine::build` end-to-end against the actual Ubuntu 24.04.4 LTS Server installer ISO — the unsquashfs/mksquashfs path the synthetic test matrix only stubs. Double-gated: `#[ignore]` plus `FORGEISO_RUN_REAL_LTS=1` env var. Cached ISO is verified against a pinned SHA-256 before the build runs, and the resulting ISO is checked for the ISO-9660 `CD001` magic. Helper script `tests/fixtures/download-real-lts.sh` populates the cache (verifies SHA-256 after download).
- **Engine rustdoc** at the crate root and on every top-level `pub mod` (autoinstall, config, error, events, iso, kickstart, orchestrator, product, report, scanner, workspace). Sets the stage for a `#![warn(missing_docs)]` lint at 1.0; current pub-item rustdoc gap reduced from 439 to 426 warnings (12 highest-blast-radius items filled first; the remaining 426 are individual fields/variants/methods to fill in subsequent rounds).

### Documentation
- Crate-root rustdoc for `forgeiso-engine` calls out `ForgeIsoEngine::{new, build, build_cancellable, inspect_source, verify_iso, scan_iso, test_iso}` as the top-level entry points and links the stability surface to `STABILITY.md` + `engine/tests/public-api.golden`.
- Module-level rustdoc cross-links to ADRs 0004 (errors), 0005 (per-distro injection), 0008 (reliability contract), 0012 (cancellation + circuit breakers) so a reader landing on any module via `cargo doc` can jump to the load-bearing decision rationale.

## [0.3.2] - 2026-05-02

### Added
- **GUI: persisted "show recommended only" filter** on Step 1 distro grid. Toggle next to the POPULAR DISTRIBUTIONS header hides cards the active configuration profile does not recommend; per-card `visible:` binding compacts the layout. State persisted via `UiState.show_recommended_only` in `slint-state.json`. Default off so first-run users still see every preset.
- **GUI: inline "Preview defaults" disclosure** on Step 1 profile picker. Expandable card below the active profile description renders every Some-field from `populate(kind)` in a labeled list. Multi-line lists collapse to commas; empty-string overrides render as `(cleared)`; bool fields render yes/no. Local UI state, not persisted.
- **GUI: compare-profiles modal** with side-by-side diff. "Compare" button at the end of the chip row opens a centered overlay listing all eight `ProfileDefaults` fields for two profiles. Differing rows tint amber via `Palette.amber-soft` (WCAG AA in both modes); A and B are switchable via chip rows inside the modal. Defaults to ServerDefault vs ServerHardened.
- **Test fixtures: synthetic squashfs payload** in Ubuntu and Mint synthetic ISOs. `tests/fixtures/synthetic-iso.sh` now fabricates a tiny 4-file rootfs and runs `mksquashfs -comp xz` to emit `casper/filesystem.squashfs` (~4 KB), so `test-releases.sh` exercises the engine's unsquashfs/mksquashfs branch instead of the warn-only "rootfs not modified" fallback. ISO stays at ~559 KB total. Helper degrades gracefully if `mksquashfs` is missing.

### Fixed
- **GUI: Configure step layout** — `vertical-stretch: 1` on the tab rail + content `HorizontalLayout` so the row claims the available middle space instead of collapsing to natural height and leaving dead air between the source banner and Continue button.

### Tests
- 7 new unit tests in `forge-slint/src/profiles.rs` covering `preview_rows()` and `compare_rows()` (every Some-field formatting case, multi-line collapse, bool/Option rendering, full 5×5 profile-pair coverage). forge-slint test count: 45 → 52. `scripts/test-releases.sh --parallel 4` still 33/33 PASS.

## [Unreleased]

### Added
- **GUI: enterprise ops-console shell** (Direction A: Datadog/Splunk/Grafana)
  - Left `Sidebar` 220px replacing top step bar; rail-accent active state, brand monogram, GitHub-style nav.
  - Bottom `StatusBar` with health-dot, version, build-hash (mono), license, error-count, theme toggle.
  - In-content `PageHeader` with current step title + subtitle + progress + doctor + cancel.
  - Two themes (dark default, light optional) reactive via `Theme.mode`; flips instantly without restart.
  - WCAG AA contrast in both modes; every status fill routed through `Palette.*-soft` tokens; no theme-blind rgba in component code.
  - 14 vector SVG icons (Lucide-style monochrome) replacing Unicode chrome glyphs.
  - Persistent theme preference at `$XDG_DATA_HOME/forgeiso/slint-state.json`.
  - 13 FCard sections in Configure step (5 tabs).
  - Build + Check steps wrapped in FCard sections (11 + 6 cards respectively).
  - Brand-colored monogram circles replace generic emoji on DistroCard.
- **Reliability: chaos test suite** at `engine/tests/chaos.rs` — 13 fault-injection scenarios (missing tool, nonzero exit, corrupt ISO, no CD001 signature, sha256 mismatch/missing, source not found, readonly output, two engines, fake silent noop, event subscriber drop, etc). Each pinned to specific `EngineError` variant.
- **Property tests** at `engine/tests/proptest_config.rs` — 13 `proptest!` blocks covering InjectConfig validators, output filename sanitization, PresetId roundtrip, output label, SHA-256 validation, IsoSource roundtrip, BuildConfig YAML parse, timezone/locale charsets, ProfileKind serde, password hashing, workspace::safe_join.
- **Mutation testing** via `cargo-mutants` — `.cargo-mutants.toml` config, `scripts/run-mutants.sh` wrapper, 21 killer tests added; 95% kill score on targeted survivors.
- **API contract test** at `engine/tests/api_contract.rs` — env-gated `FORGEISO_RUN_API_CONTRACT=1` golden file at `engine/tests/public-api.golden` (5755 LOC, 3083 pub items, 1122 impls). `scripts/regenerate-api-golden.sh` for intentional updates.
- **Perf regression gate** — `scripts/perf-bench.sh` (bench/compare/capture) + `tests/baseline-perf.json` (4 captured benches: sha256 4MiB ~15.8 ms, generators ~6.5 ms each).
- **Single-command S+ audit** at `scripts/s-tier-audit.sh` — fmt + lint + test + coverage + build + perf + security + docs presence (8 dimensions). `--fast` skips slow steps for pre-commit.
- **9 ADRs**:
  - 0001 four-crate workspace
  - 0002 xorriso over alternatives
  - 0003 three-front-ends strategy
  - 0004 thiserror EngineError + anyhow at boundaries
  - 0005 distro-specific autoinstall injection
  - 0006 post-decomposition module taxonomy
  - 0007 seven-layer testing strategy
  - 0008 reliability contract for desktop tool
  - 0009 spec-driven development workflow
- **Documentation**: `docs/ARCHITECTURE.md`, `docs/SLO.md`, `docs/COMPLIANCE.md` (NIST 800-53 + CIS v8 + SOC 2), `docs/CHAOS.md`, `docs/CI-INTEGRATION.md`, `docs/MUTATION.md`, `docs/REPORT-CARD-2026-05-01.md`, `docs/specs/README.md` (workflow scaffold).

### Changed
- **Engine decomposed**: every non-test source file ≤500 LOC after Phase 1+7 splits.
  - `engine/src/config/inject.rs` 2034 → `inject/{mod, validate/{identity,system,packages,network,ssh,storage,grub,output}, tests/{...}}` (17 files all ≤350)
  - `engine/src/autoinstall/ubuntu.rs` 2005 → `ubuntu/{generate, merge, tests, mod}`
  - `engine/src/sources.rs` 1123 → `sources/{catalog/{ubuntu,fedora,arch,mint,debian,popos,opensuse,rhel_family}, preset_id, preset, strategy, mod}`
  - `engine/src/vm.rs` 991 → `vm/{launch, qemu, spec, hypervisor, proxmox, hyperv, vmware, vbox, firmware, ovmf, mod}` (11 files all <500)
  - `engine/src/orchestrator/helpers.rs` 745 → `helpers/{archinstall, boot_patch, cache, host, paths, process, mod}`
  - `engine/src/autoinstall/late_commands.rs` 677 → `late_commands/{time_users, system, packages, finalize, mod}`
  - `engine/src/kickstart.rs` 616 → `kickstart/{header, post, cidr, mod}`
  - `engine/src/orchestrator/inject.rs` 615 → `inject/{configure, place, mod}`
  - `engine/src/config/inject_builder.rs` 708 → `inject_builder/{identity, system, network, storage, packages, services, tests, mod}`
- **CLI decomposed**: `cli/src/main.rs` 743 → `main` 30 + `cli` 441 + `dispatch` 221 + `preset` 84.
- **TUI decomposed**: `tui/src/state.rs` 849 → `state/{mod, nav, fields, source, build, configure}`. `tui/src/main.rs` 518 → `main` 144 + `keymap` 367 + `runtime` 60.
- **GUI decomposed**: `forge-slint/src/main.rs` 901 → `main` 408 + `handlers/{common, source, configure, build, check, mod}`. `forge-slint/ui/steps/configure.slint` 966 → top 497 + per-tab files (109-193 LOC each).
- **Coverage gate ratcheted**: `tarpaulin.toml` `fail-under` 40 → 64 (measured 69.84% library coverage; 127 new tests in scanner/helpers/diff/doctor/scan/verify/iso/product/workspace/vm/events/build/inject/configure/place).
- **CLAUDE.md** gains Module-layout section, Observability/Runbooks/ADR/Benchmarks pointers, perf-bench script, s-tier-audit script.
- Local `cargo update -p rustls-webpki` (0.103.9 → 0.103.13) and `cargo update -p tar` (0.4.44 → 0.4.45) confirmed all 6 reported RUSTSEC advisories (2026-0049/0098/0099/0104 webpki, 2026-0067/0068 tar) clear with patched transitive versions. Cargo.lock is gitignored per project policy, so CI re-resolves; if a future audit re-flags these, run the same `cargo update` locally and verify before tagging.

### Removed
- `forge-slint/ui/components/header.slint` and `step_bar.slint` — replaced by `PageHeader` + `Sidebar` in the enterprise ops-console shell.

### Test count progression
- Baseline (pre-uplift): 670
- Post-Phase-1 decomposition: 705
- Post-Phase-4 (proptest + contract + mutation): 705 + 13 + 1 + 21 = 740
- Post-Phase-7 (chaos + coverage + final decomp): 839+

### Tier (per `~/.claude/TIER_RUBRIC.md`, see `docs/REPORT-CARD-2026-05-01.md`)
- Pre-uplift: B
- Post-uplift: A floor (with Phase 7 chaos: B+ → A on Reliability)
- Code quality A → A+ (every non-test file ≤568 LOC, vast majority ≤300)
- Testing A+ → S (chaos + property + mutation + contract + golden + perf-gate)
- Documentation A+ → S+ (9 ADRs + 5 new top-level docs + spec scaffold)
- Process A → S+ (ADR 0009 codifies brainstorm → spec → ADR → plan → review → implement)

## [0.1.0] — 2026-04 (initial)

Pre-changelog history. Reconstruct from `git log` if needed. Key milestones:
- Four-crate workspace (engine + cli + tui + forge-slint)
- Distro support: Ubuntu/Mint cloud-init/preseed, Fedora/RHEL kickstart, Arch archinstall
- xorriso ISO repack pipeline
- SHA-256 verification + ISO-9660 compliance check
- Engine event broadcast bus
- CLI + TUI + Slint GUI sharing the same engine

[Unreleased]: https://github.com/Al-Sarraf-Tech/ForgeISO/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Al-Sarraf-Tech/ForgeISO/releases/tag/v0.1.0
