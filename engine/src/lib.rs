//! # ForgeISO engine
//!
//! Library crate behind the `forgeiso` CLI, the `forgeiso-tui` TUI, and
//! the `forge-slint` desktop GUI. The engine is the single source of
//! truth for ISO inspection, autoinstall/kickstart/preseed generation,
//! ISO repacking, and the post-build verify/scan/test pipeline.
//!
//! Front-ends never invoke external tools or touch the filesystem
//! directly; they construct an [`InjectConfig`] (declarative description
//! of what to inject) plus a [`BuildConfig`] (build-time options),
//! then drive a [`ForgeIsoEngine`] instance and render the streaming
//! [`EngineEvent`] feed.
//!
//! ## Top-level entry points
//!
//! - [`ForgeIsoEngine::new`] — construct an engine.
//! - [`ForgeIsoEngine::build`] — produce a repacked ISO from a
//!   [`BuildConfig`] and an output directory.
//! - [`ForgeIsoEngine::build_cancellable`] — same, with a
//!   [`tokio_util::sync::CancellationToken`] for cooperative
//!   cancellation.
//! - [`ForgeIsoEngine::inspect_source`] / [`ForgeIsoEngine::verify`]
//!   / [`ForgeIsoEngine::scan`] / [`ForgeIsoEngine::test_iso`] —
//!   read-only / verify / scan / boot-test surfaces.
//!
//! ## Stability
//!
//! Items re-exported at the crate root form the public stability
//! surface. The set is captured by `engine/tests/public-api.golden`
//! and changes to it are reviewed against the project's
//! [`STABILITY.md`](https://github.com/Al-Sarraf-Tech/ForgeISO/blob/main/STABILITY.md)
//! commitment. Items reachable only through `pub(crate)` paths are
//! internal and may change at any time.
//!
//! ## Errors
//!
//! Every fallible operation returns [`EngineResult<T>`] —
//! `Result<T, EngineError>`. The error taxonomy is documented on
//! [`EngineError`] and each variant maps to one of the documented
//! exit codes in `docs/RUNBOOKS.md`.

pub mod autoinstall;
pub mod config;
pub mod error;
pub mod events;
pub mod iso;
pub mod kickstart;
pub mod mint_preseed;
pub mod observability;
pub mod orchestrator;
pub mod product;
pub mod profiles;
pub mod report;
pub mod scanner;
pub mod sources;
pub mod vm;
pub mod workspace;

pub use autoinstall::{
    build_feature_late_commands, generate_autoinstall_yaml, hash_password, merge_autoinstall_yaml,
};
pub use config::{
    BuildConfig, ContainerConfig, Distro, FirewallConfig, GrubConfig, InjectConfig,
    InjectConfigBuilder, IsoSource, NetworkConfig, ProfileKind, ProxyConfig, ScanPolicy, SshConfig,
    SwapConfig, TestingPolicy, ToolStatus, UserConfig,
};
pub use error::{EngineError, EngineResult};
pub use events::{EngineEvent, EventKind, EventLevel, EventPhase};
pub use iso::{BootSupport, IsoMetadata, SourceKind};
pub use kickstart::generate_kickstart_cfg;
pub use mint_preseed::generate_mint_preseed;
pub use orchestrator::{
    BuildResult, DiffEntry, DoctorReport, ForgeIsoEngine, Iso9660Compliance, IsoDiff, ScanResult,
    TestResult, VerifyResult,
};
pub use product::{GuidedWorkflowProgress, GuidedWorkflowStep};
pub use profiles::{Profile, ProfileCatalog};
pub use sources::{
    all_presets, find_preset, find_preset_by_str, resolve_url, AcquisitionStrategy, IsoPreset,
    PresetId,
};
pub use vm::{
    create_qemu_disk, emit_launch, find_ovmf, maybe_remove_kvm, ovmf_candidates, proxmox_cmds,
    qemu_bios_args, qemu_uefi_args, vbox_commands, vmware_instructions, FirmwareMode, Hypervisor,
    VmLaunchOutput, VmLaunchSpec,
};
