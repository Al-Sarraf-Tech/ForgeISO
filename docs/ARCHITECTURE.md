# ForgeISO Architecture

A bird's-eye view of how ForgeISO is built. For specific decision rationale,
see the ADRs in `docs/adr/`. For operational guidance, see `docs/RUNBOOKS.md`.

## Mission

Inject unattended-install configuration into a Linux distro ISO so the
resulting ISO boots, partitions disks, and installs a fully-configured system
with zero operator interaction. Supports Ubuntu/Mint cloud-init/preseed,
Fedora/RHEL kickstart, Arch archinstall, and the RHEL-derivative family.

## Crate layout

```
ForgeISO/
├── engine/           Library crate. Pure logic. No UI, no shell-out at the
│                     library boundary (orchestrator does the shelling).
├── cli/              Binary crate. clap-derive command surface.
├── tui/              Binary crate. Ratatui terminal UI.
└── forge-slint/      Binary crate. Slint desktop GUI.
```

ADR 0001 documents the four-crate split. ADR 0003 documents the three-
front-end strategy (CLI + TUI + GUI sharing the same engine).

## Engine module map (post-decomposition, ADR 0006)

```
engine/src/
├── config/
│   ├── inject/
│   │   ├── mod.rs                     InjectConfig struct + 1-line validate()
│   │   ├── validate/
│   │   │   ├── mod.rs                 orchestrator preserves original order
│   │   │   ├── identity.rs            hostname/user/realname/tz/locale/kb
│   │   │   ├── system.rs              user/services/firewall/sysctl
│   │   │   ├── packages.rs            apt/dnf/pacman repos+mirrors
│   │   │   ├── network.rs             proxy/static_ip/gateway/DNS/NTP
│   │   │   ├── ssh.rs                 keys + containers
│   │   │   ├── storage.rs             swap/mounts/encryption/wallpaper
│   │   │   ├── grub.rs                bootloader timeout/cmdline/default
│   │   │   └── output.rs              out_name/output_label/sha256
│   │   └── tests/                     per-concern test modules
│   ├── inject_builder.rs              typed builder for InjectConfig
│   └── build.rs                       BuildConfig (YAML driver)
├── autoinstall/
│   ├── ubuntu/                        cloud-init / autoinstall yaml
│   │   ├── generate.rs                generate_autoinstall_yaml
│   │   ├── merge.rs                   merge user yaml into ours
│   │   └── tests.rs                   integration tests
│   └── late_commands/                 hook generation per-feature
│       ├── time_users.rs
│       ├── system.rs
│       ├── packages.rs
│       └── finalize.rs
├── kickstart/                         Fedora/RHEL kickstart cfg
│   ├── header.rs
│   ├── post.rs
│   └── cidr.rs
├── mint_preseed.rs                    Mint preseed
├── sources/                           ISO source presets
│   ├── catalog.rs                     ALL_PRESETS static list
│   ├── preset_id.rs                   typed PresetId enum
│   ├── preset.rs                      IsoPreset struct
│   └── strategy.rs                    AcquisitionStrategy enum
├── vm/                                VM launch dispatch
│   ├── mod.rs                         VmLaunchSpec dispatch
│   ├── qemu.rs                        BIOS/UEFI args, KVM, qemu-img
│   ├── proxmox.rs / hyperv.rs / vmware.rs / vbox.rs
│   ├── firmware.rs / ovmf.rs          firmware variant selection
│   ├── spec.rs                        VmLaunchSpec struct
│   └── hypervisor.rs                  Hypervisor enum
├── orchestrator/                      shell-out + workflow dispatch
│   ├── helpers/                       subprocess + path + cache
│   │   ├── archinstall.rs / boot_patch.rs / cache.rs / host.rs
│   │   ├── paths.rs / process.rs
│   ├── inject/                        per-distro injection workflow
│   │   ├── configure.rs               apply InjectConfig to extracted ISO tree
│   │   └── place.rs                   put files in the right paths per distro
│   └── (sha256 + scan helpers)
├── events.rs                          EngineEvent + EventPhase (broadcast bus)
├── error.rs                           EngineError taxonomy (E001-E100)
├── workspace.rs                       safe_join + workspace lock
├── product.rs                         output product metadata
├── report.rs                          build report struct
├── scanner.rs                         ISO content scan
├── iso.rs                             ISO-9660 read/write helpers
└── lib.rs                             pub use surface
```

## CLI module map

```
cli/src/
├── main.rs             clap dispatch (≤30 LOC)
├── cli.rs              clap-derive Command + Arg structs
├── dispatch.rs         per-subcommand handlers
└── preset.rs           preset resolution helpers
```

## TUI module map

```
tui/src/
├── main.rs             ratatui run loop
├── state/
│   ├── mod.rs          App struct + log push
│   ├── nav.rs          WizardStep / SourceFocus / ConfigTab / WorkerMsg
│   ├── fields.rs       FieldDef + FieldKind
│   ├── source.rs       effective_source / resolve_distro
│   ├── build.rs        build_is_complete / invalidate_*
│   └── configure.rs    field index switches + InjectConfig builder
├── ui/                 ratatui draw functions
└── worker.rs           background tokio task pump
```

## GUI module map

```
forge-slint/
├── src/
│   ├── main.rs         orchestration only (≤410 LOC)
│   ├── handlers/
│   │   ├── mod.rs      wire_all_handlers(&AppWindow)
│   │   ├── common.rs   cancel/theme/doctor/log/step-bar
│   │   ├── source.rs   preset/browse/source-changed/continue/clear
│   │   ├── configure.rs browse-output/continue/back/defaults/edited
│   │   ├── build.rs    back/run/view-results
│   │   └── check.rs    verify/iso9660/copy-sha256/open-folder
│   ├── app.rs          ForgeApp + thread-local APP cell
│   ├── state.rs        InjectState + VerifyState + UiState (persisted)
│   ├── persist.rs      atomic JSON write
│   ├── config.rs       PresetCard + handle_preset_clicked
│   ├── defaults.rs     distro-aware default fields
│   ├── jobs.rs         build / scan / inject / verify task dispatch
│   ├── worker.rs       file picker + ISO detect spawn
│   └── obs.rs          tracing-subscriber JSON init
└── ui/
    ├── theme.slint                global Theme + Palette + Sizes + Fonts
    ├── app.slint                  AppWindow root
    ├── globals/
    │   ├── app_state.slint        AppState (navigation + job lifecycle)
    │   └── form_state.slint       FormState (every input field)
    ├── components/
    │   ├── design_system.slint    F* primitives (FButton, FCard, etc.)
    │   ├── icon.slint             FIcon* wrappers around 14 SVG icons
    │   ├── sidebar.slint          left-rail nav (replaces top StepBar)
    │   ├── page_header.slint      slim title bar inside content
    │   ├── status_bar.slint       footer (version + build + theme toggle)
    │   └── log_panel.slint        right-side activity log (collapsible)
    ├── steps/
    │   ├── source.slint           Step 1 (DistroCard grid + browse)
    │   ├── configure.slint        Step 2 (TabRail dispatch)
    │   ├── configure/
    │   │   ├── tab_identity.slint   IDENTITY + OUTPUT FCards
    │   │   ├── tab_access.slint     SSH + USER ACCESS + NETWORK
    │   │   ├── tab_system.slint     SYSTEM + PACKAGES
    │   │   ├── tab_services.slint   SERVICES + CONTAINERS + FIREWALL
    │   │   └── tab_storage.slint    STORAGE + BOOTLOADER + CMDS + DESKTOP
    │   ├── build.slint            Step 3 (settings recap + run)
    │   └── check.slint            Step 4 (verify checksum + ISO-9660)
    └── icons/                     14 Lucide-style monochrome SVGs
```

## Cross-cutting concerns

### Observability (CLAUDE.md "Observability" section)
- All three frontends call `obs::init_tracing()` at startup
- Daily-rolling JSON files at `<FORGEISO_LOG_DIR or $XDG_STATE_HOME/forgeiso>/forgeiso{,-tui,-gui}.log.<date>`
- `RUST_LOG` env var overrides default `info` level
- `docs/METRICS.md` (if present) describes log-to-metrics pipelines
- `docs/SLO.md` codifies per-op SLOs

### Reliability (ADR 0008)
- Every shell-out has a clear failure path (MissingTool / Subprocess / timeout)
- Cancellation honored ≤1 second
- Output integrity atomic (tmp+rename in same dir)
- Form state persistence power-loss-safe

### Testing (ADR 0007 — seven layers)
- Unit (`#[cfg(test)] mod tests` next to code)
- Integration (`engine/tests/`, `cli/tests/`, etc.)
- E2E regression (`engine/tests/e2e_regression.rs`)
- Distro regression (`engine/tests/distro_regression.rs`)
- Property (`engine/tests/proptest_config.rs` — 13 properties)
- Mutation (cargo-mutants — 95% kill on targeted survivors)
- Contract (`engine/tests/api_contract.rs` — 3083 pub items golden)
- Plus chaos (`engine/tests/chaos.rs` — 13 fault-injection scenarios)
- Plus coverage gate (`tarpaulin.toml`)
- Plus perf gate (`scripts/perf-bench.sh`)

### Security (docs/COMPLIANCE.md)
- cargo-audit + cargo-deny + gitleaks + Trivy fs scan + syft SBOM in CI
- SHA-256 verification of source + output
- Path-safety validation (`EngineError::PathSafety` E050)
- Passwords + LUKS passphrase never persisted (`#[serde(skip)]`)
- NIST 800-53 / CIS v8 / SOC 2 control mapping documented

### Process
- Conventional commits enforced
- 8 ADRs documenting load-bearing decisions
- `scripts/s-tier-audit.sh` single-command gate
- branch protection on Al-Sarraf-Tech (CLAUDE.md absolute)

## Module boundaries

The engine is a pure library. The three frontends import it. Frontend ↔
frontend imports are forbidden.

```
engine ←—— cli
       ←—— tui
       ←—— forge-slint
```

The engine's public API is locked by `engine/tests/public-api.golden`
(ADR 0007 contract layer). Changing it requires regenerating the golden
+ writing an ADR.

## Where to make changes

| Change | Where |
|---|---|
| Add a distro | engine/src/sources/catalog.rs + engine/src/autoinstall/<distro>/ + engine/src/config/inject/validate/<concern>.rs |
| Add a config field | engine/src/config/inject/mod.rs (struct) + per-concern validator + each frontend's form |
| Add an EngineError | engine/src/error.rs + chaos test for it (engine/tests/chaos.rs) + docs/RUNBOOKS.md entry |
| Add a GUI step | forge-slint/ui/steps/<step>.slint + forge-slint/src/handlers/<step>.rs + handlers/mod.rs wire_all_handlers |
| Add a CLI subcommand | cli/src/cli.rs (clap) + cli/src/dispatch.rs (handler) |
| Add a load-bearing decision | docs/adr/000N-<title>.md + add to docs/adr/README.md |

## Reference

- ADR 0001 four-crate workspace
- ADR 0002 xorriso over alternatives
- ADR 0003 three-front-end strategy
- ADR 0004 thiserror EngineError + anyhow at boundaries
- ADR 0005 distro-specific autoinstall injection
- ADR 0006 post-decomposition module taxonomy
- ADR 0007 seven-layer testing
- ADR 0008 reliability contract for desktop tool
- ADR 0009 spec-driven development workflow (this push)
