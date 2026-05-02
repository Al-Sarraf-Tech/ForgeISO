//! Configuration profile catalog stub.
//!
//! The full `ProfileCatalog` and `ProfileKind` variants for `ServerDefault`,
//! `ServerHardened`, `DesktopDeveloper`, `Kiosk`, and `MinimalCloud` land
//! in `engine/src/profiles/` via a parallel agent. Until that lands, this
//! module mirrors the same surface so the UI can render the picker, the
//! recommended-badge, and the profile-driven default population.
//!
//! When the engine catalog ships, swap the local `ProfileKind`, the
//! `recommended_for` mapping, and `populate_defaults` body to delegate to
//! `forgeiso_engine::profiles::ProfileCatalog`.
//!
//! Serialization names match what `engine` will use (`serde(rename_all =
//! "kebab-case")`-style strings) so the persisted UI state will not need a
//! migration when the engine module ships.

use std::collections::HashSet;

use crate::defaults::DistroDefaults;

/// One of the five canonical configuration profiles.
///
/// These string identifiers must remain stable — they are persisted in the
/// Slint `FormState.selected-profile` property and round-trip through the
/// JSON state file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    ServerDefault,
    ServerHardened,
    DesktopDeveloper,
    Kiosk,
    MinimalCloud,
}

impl ProfileKind {
    /// All five variants in display order (matches the chip row).
    pub const ALL: [ProfileKind; 5] = [
        ProfileKind::ServerDefault,
        ProfileKind::ServerHardened,
        ProfileKind::DesktopDeveloper,
        ProfileKind::Kiosk,
        ProfileKind::MinimalCloud,
    ];

    /// Stable serialization id used by `FormState.selected-profile`.
    pub fn as_id(self) -> &'static str {
        match self {
            ProfileKind::ServerDefault => "server-default",
            ProfileKind::ServerHardened => "server-hardened",
            ProfileKind::DesktopDeveloper => "desktop-developer",
            ProfileKind::Kiosk => "kiosk",
            ProfileKind::MinimalCloud => "minimal-cloud",
        }
    }

    /// Parse the stable id back into a profile kind.
    pub fn from_id(id: &str) -> Option<Self> {
        ProfileKind::ALL.iter().copied().find(|p| p.as_id() == id)
    }

    /// Default profile when nothing else is selected.
    pub fn default_kind() -> Self {
        ProfileKind::ServerDefault
    }

    /// Returns true if this profile is the recommended pairing for the
    /// given preset id. When the engine catalog ships, this will delegate
    /// to `forgeiso_engine::profiles::ProfileKind::recommended_for`.
    // Badge wiring lands in the next commit; the recommended_for mapping is
    // already covered by unit tests so it is exercised at build time.
    #[allow(dead_code)]
    pub fn recommended_for(self, preset_id: &str) -> bool {
        match self {
            ProfileKind::ServerDefault => matches!(
                preset_id,
                "ubuntu-server-lts"
                    | "ubuntu-server-jammy"
                    | "fedora-server"
                    | "rocky-linux"
                    | "almalinux"
                    | "centos-stream"
            ),
            ProfileKind::ServerHardened => matches!(
                preset_id,
                "rocky-linux" | "almalinux" | "centos-stream" | "fedora-server"
            ),
            ProfileKind::DesktopDeveloper => {
                matches!(preset_id, "linux-mint-cinnamon" | "arch-linux")
            }
            ProfileKind::Kiosk => matches!(preset_id, "linux-mint-cinnamon"),
            ProfileKind::MinimalCloud => matches!(
                preset_id,
                "ubuntu-server-lts" | "ubuntu-server-jammy" | "fedora-server"
            ),
        }
    }
}

/// One entry in the profile chip row.
#[derive(Debug, Clone, Copy)]
pub struct ProfileMeta {
    pub kind: ProfileKind,
    /// Short label shown inside the chip (uppercase rendering done in slint).
    pub label: &'static str,
    /// One-line description shown under the chip row when this profile is
    /// selected.
    pub description: &'static str,
}

/// Static catalog of all five profiles, in display order.
pub const PROFILE_META: [ProfileMeta; 5] = [
    ProfileMeta {
        kind: ProfileKind::ServerDefault,
        label: "SERVER DEFAULT",
        description:
            "Balanced server baseline: SSH on, firewall deny-by-default, common admin tools.",
    },
    ProfileMeta {
        kind: ProfileKind::ServerHardened,
        label: "SERVER HARDENED",
        description:
            "Security-first server: no password SSH, sysctl hardening, audit-ready, minimal packages.",
    },
    ProfileMeta {
        kind: ProfileKind::DesktopDeveloper,
        label: "DESKTOP",
        description:
            "Developer workstation: editor, runtimes, container tooling, sudo without password prompt.",
    },
    ProfileMeta {
        kind: ProfileKind::Kiosk,
        label: "KIOSK",
        description:
            "Single-purpose appliance: auto-login, locked-down shell, no extra services.",
    },
    ProfileMeta {
        kind: ProfileKind::MinimalCloud,
        label: "MINIMAL CLOUD",
        description:
            "Smallest viable cloud image: cloud-init only, no extras, ready for golden-image automation.",
    },
];

/// Profile-derived overrides on top of the per-distro `DistroDefaults`.
///
/// Each `Option` field replaces the corresponding distro default when set.
/// Empty `Some(String::new())` is a deliberate "clear this field" signal.
#[derive(Debug, Clone, Default)]
pub struct ProfileDefaults {
    pub packages: Option<String>,
    pub enable_services: Option<String>,
    pub disable_services: Option<String>,
    pub firewall_policy: Option<String>,
    pub allow_ports: Option<String>,
    pub sysctl_pairs: Option<String>,
    pub ssh_password_auth: Option<bool>,
    pub sudo_nopasswd: Option<bool>,
}

/// Stub `populate` — returns the profile-shaped overrides for the given
/// profile, layered on top of whatever distro defaults are already in the
/// form. The full engine ProfileCatalog will return the same `ProfileDefaults`
/// shape but with its own richer mapping per distro.
pub fn populate(kind: ProfileKind) -> ProfileDefaults {
    match kind {
        ProfileKind::ServerDefault => ProfileDefaults {
            firewall_policy: Some("deny".into()),
            allow_ports: Some("22/tcp".into()),
            ssh_password_auth: Some(false),
            ..Default::default()
        },
        ProfileKind::ServerHardened => ProfileDefaults {
            packages: Some("curl wget git vim htop rsync auditd fail2ban".into()),
            disable_services: Some("avahi-daemon\ncups".into()),
            firewall_policy: Some("deny".into()),
            allow_ports: Some("22/tcp".into()),
            ssh_password_auth: Some(false),
            sudo_nopasswd: Some(false),
            sysctl_pairs: Some(
                "net.ipv4.conf.all.rp_filter=1\nkernel.kptr_restrict=2\nkernel.dmesg_restrict=1"
                    .into(),
            ),
            ..Default::default()
        },
        ProfileKind::DesktopDeveloper => ProfileDefaults {
            packages: Some("curl git vim build-essential htop tmux jq".into()),
            firewall_policy: Some("allow".into()),
            sudo_nopasswd: Some(true),
            ..Default::default()
        },
        ProfileKind::Kiosk => ProfileDefaults {
            packages: Some("curl".into()),
            disable_services: Some("cups\navahi-daemon\nbluetooth".into()),
            firewall_policy: Some("deny".into()),
            allow_ports: Some(String::new()),
            ssh_password_auth: Some(false),
            ..Default::default()
        },
        ProfileKind::MinimalCloud => ProfileDefaults {
            packages: Some(String::new()),
            enable_services: Some("cloud-init".into()),
            firewall_policy: Some("deny".into()),
            allow_ports: Some("22/tcp".into()),
            ssh_password_auth: Some(false),
            ..Default::default()
        },
    }
}

/// Layer profile overrides over distro defaults, then return only the fields
/// that should actually change for unedited fields.
///
/// Mirrors the calling convention of `defaults::apply_defaults`: returns a
/// `Vec<(field, value)>` so the caller can fan out to FormState setters
/// without binding to engine types.
pub fn apply_profile_overrides(
    base: &DistroDefaults,
    profile: ProfileKind,
    edited: &HashSet<String>,
) -> Vec<(&'static str, String)> {
    let overrides = populate(profile);
    let mut changes: Vec<(&'static str, String)> = Vec::new();

    let mut maybe_push = |field: &'static str, value: Option<String>| {
        if edited.contains(field) {
            return;
        }
        if let Some(v) = value {
            changes.push((field, v));
        }
    };

    maybe_push(
        "packages",
        overrides.packages.or_else(|| Some(base.packages.clone())),
    );
    maybe_push(
        "enable_services",
        overrides
            .enable_services
            .or_else(|| Some(base.enable_services.clone())),
    );
    maybe_push(
        "disable_services",
        overrides
            .disable_services
            .or_else(|| Some(base.disable_services.clone())),
    );
    maybe_push(
        "firewall_policy",
        overrides
            .firewall_policy
            .or_else(|| Some(base.firewall_policy.clone())),
    );
    maybe_push(
        "allow_ports",
        overrides
            .allow_ports
            .or_else(|| Some(base.allow_ports.clone())),
    );
    maybe_push("sysctl_pairs", overrides.sysctl_pairs);

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_round_trips_for_all_kinds() {
        for kind in ProfileKind::ALL {
            assert_eq!(ProfileKind::from_id(kind.as_id()), Some(kind));
        }
    }

    #[test]
    fn unknown_id_returns_none() {
        assert_eq!(ProfileKind::from_id("not-a-profile"), None);
    }

    #[test]
    fn default_is_server_default() {
        assert_eq!(ProfileKind::default_kind(), ProfileKind::ServerDefault);
        assert_eq!(ProfileKind::default_kind().as_id(), "server-default");
    }

    #[test]
    fn server_default_recommended_for_servers() {
        assert!(ProfileKind::ServerDefault.recommended_for("ubuntu-server-lts"));
        assert!(ProfileKind::ServerDefault.recommended_for("rocky-linux"));
        assert!(!ProfileKind::ServerDefault.recommended_for("linux-mint-cinnamon"));
    }

    #[test]
    fn kiosk_only_recommended_for_mint() {
        assert!(ProfileKind::Kiosk.recommended_for("linux-mint-cinnamon"));
        assert!(!ProfileKind::Kiosk.recommended_for("ubuntu-server-lts"));
        assert!(!ProfileKind::Kiosk.recommended_for("arch-linux"));
    }

    #[test]
    fn desktop_developer_recommended_for_desktop_presets() {
        assert!(ProfileKind::DesktopDeveloper.recommended_for("linux-mint-cinnamon"));
        assert!(ProfileKind::DesktopDeveloper.recommended_for("arch-linux"));
        assert!(!ProfileKind::DesktopDeveloper.recommended_for("fedora-server"));
    }

    #[test]
    fn populate_returns_overrides_per_kind() {
        let hardened = populate(ProfileKind::ServerHardened);
        assert_eq!(hardened.ssh_password_auth, Some(false));
        assert!(hardened
            .sysctl_pairs
            .as_deref()
            .is_some_and(|v| v.contains("rp_filter")));

        let kiosk = populate(ProfileKind::Kiosk);
        assert_eq!(kiosk.allow_ports.as_deref(), Some(""));
    }

    #[test]
    fn apply_profile_overrides_skips_edited_fields() {
        let base = DistroDefaults {
            packages: "curl".into(),
            firewall_policy: "deny".into(),
            allow_ports: "22/tcp".into(),
            ..Default::default()
        };
        let mut edited = HashSet::new();
        edited.insert("packages".into());
        let changes = apply_profile_overrides(&base, ProfileKind::ServerHardened, &edited);
        assert!(changes.iter().all(|(name, _)| *name != "packages"));
        // sysctl_pairs always comes from the profile (not in distro defaults)
        assert!(changes.iter().any(|(name, _)| *name == "sysctl_pairs"));
    }

    #[test]
    fn apply_profile_overrides_includes_base_when_profile_silent() {
        let base = DistroDefaults {
            packages: "curl wget".into(),
            ..Default::default()
        };
        let edited = HashSet::new();
        let changes = apply_profile_overrides(&base, ProfileKind::ServerDefault, &edited);
        let pkgs = changes
            .iter()
            .find(|(name, _)| *name == "packages")
            .map(|(_, v)| v.clone());
        assert_eq!(pkgs, Some("curl wget".into()));
    }

    #[test]
    fn profile_meta_table_covers_all_kinds() {
        assert_eq!(PROFILE_META.len(), ProfileKind::ALL.len());
        for (meta, kind) in PROFILE_META.iter().zip(ProfileKind::ALL.iter()) {
            assert_eq!(meta.kind, *kind);
            assert!(!meta.label.is_empty());
            assert!(!meta.description.is_empty());
        }
    }
}
