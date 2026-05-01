//! ISO source presets and acquisition strategies.
//!
//! Split into:
//! - [`preset_id`] — the [`PresetId`] enum and string parsing/rendering.
//! - [`strategy`] — the [`AcquisitionStrategy`] enum.
//! - [`preset`] — the [`IsoPreset`] struct and lookup/format helpers.
//! - [`catalog`] — the static built-in preset catalog.

mod catalog;
mod preset;
mod preset_id;
mod strategy;

pub use preset::{
    all_presets, find_preset, find_preset_by_str, format_preset_detail, format_preset_summary,
    resolve_url, IsoPreset,
};
pub use preset_id::PresetId;
pub use strategy::AcquisitionStrategy;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_returns_thirty_five_items() {
        assert_eq!(all_presets().len(), 35);
    }

    #[test]
    fn find_preset_by_str_lowercase() {
        let preset = find_preset_by_str("ubuntu-server-lts");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, PresetId::UbuntuServerLts);
    }

    #[test]
    fn find_preset_by_str_uppercase() {
        let preset = find_preset_by_str("UBUNTU-SERVER-LTS");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, PresetId::UbuntuServerLts);
    }

    #[test]
    fn find_preset_by_str_mixed_case() {
        let preset = find_preset_by_str("Rocky-Linux");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().id, PresetId::RockyLinux);
    }

    #[test]
    fn find_preset_by_str_unknown_returns_none() {
        assert!(find_preset_by_str("does-not-exist").is_none());
    }

    #[test]
    fn resolve_url_direct_url_returns_some() {
        let preset = find_preset_by_str("ubuntu-server-lts").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_some());
        assert!(url.unwrap().contains("mirror.xenyth.net"));
    }

    #[test]
    fn resolve_url_rocky_linux_returns_some() {
        let preset = find_preset_by_str("rocky-linux").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_some());
        assert!(url.unwrap().contains("rockylinux.org"));
    }

    #[test]
    fn resolve_url_user_provided_returns_none() {
        let preset = find_preset_by_str("rhel-custom").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_none());
    }

    #[test]
    fn resolve_url_discovery_page_returns_none() {
        let preset = find_preset_by_str("manjaro").unwrap();
        let url = resolve_url(preset).unwrap();
        assert!(url.is_none());
    }

    #[test]
    fn format_preset_summary_contains_id_and_distro() {
        let preset = find_preset_by_str("arch-linux").unwrap();
        let summary = format_preset_summary(preset);
        assert!(summary.contains("arch-linux"));
        assert!(summary.contains("arch"));
    }

    #[test]
    fn format_preset_detail_contains_all_fields() {
        let preset = find_preset_by_str("ubuntu-server-lts").unwrap();
        let detail = format_preset_detail(preset);
        assert!(detail.contains("ubuntu-server-lts"));
        assert!(detail.contains("ubuntu"));
        assert!(detail.contains("server-lts"));
        assert!(detail.contains("direct_url"));
        assert!(detail.contains("releases.ubuntu.com"));
    }

    #[test]
    fn preset_id_as_str_round_trips() {
        for preset in all_presets() {
            let s = preset.id.as_str();
            let parsed = PresetId::parse(s);
            assert!(parsed.is_some(), "failed to round-trip: {s}");
            assert_eq!(&parsed.unwrap(), &preset.id);
        }
    }

    #[test]
    fn all_direct_url_presets_have_url() {
        for preset in all_presets() {
            if preset.strategy == AcquisitionStrategy::DirectUrl {
                assert!(
                    preset.direct_url.is_some(),
                    "DirectUrl preset '{}' missing direct_url",
                    preset.id.as_str()
                );
            }
        }
    }

    #[test]
    fn all_presets_have_nonempty_official_page() {
        for preset in all_presets() {
            assert!(
                !preset.official_page.is_empty(),
                "preset '{}' missing official_page",
                preset.id.as_str()
            );
        }
    }

    #[test]
    fn all_presets_have_nonempty_note() {
        for preset in all_presets() {
            assert!(
                !preset.note.is_empty(),
                "preset '{}' missing note",
                preset.id.as_str()
            );
        }
    }

    #[test]
    fn find_preset_returns_correct_struct() {
        let p = find_preset_by_str("ubuntu-server-lts").expect("should find preset");
        assert_eq!(p.id.as_str(), "ubuntu-server-lts");
        assert_eq!(p.strategy, AcquisitionStrategy::DirectUrl);
    }

    #[test]
    fn rhel_custom_is_user_provided() {
        let p = find_preset_by_str("rhel-custom").expect("rhel-custom preset must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::UserProvided);
        assert!(resolve_url(p).unwrap().is_none());
    }

    #[test]
    fn discovery_page_presets_resolve_to_none() {
        // manjaro uses a discovery page — build-stamped filenames prevent a stable direct URL
        let p = find_preset_by_str("manjaro").expect("manjaro must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::DiscoveryPage);
        assert!(
            resolve_url(p).unwrap().is_none(),
            "manjaro should resolve to None"
        );
    }

    #[test]
    fn format_preset_summary_width_is_consistent() {
        // Summary should not panic for any preset; spot-check content
        for preset in all_presets() {
            let s = format_preset_summary(preset);
            assert!(s.contains(preset.id.as_str()));
            assert!(s.contains(preset.strategy.as_str()));
        }
    }

    #[test]
    fn format_preset_detail_contains_official_page() {
        for preset in all_presets() {
            let d = format_preset_detail(preset);
            assert!(
                d.contains(preset.official_page),
                "detail missing official_page for {}",
                preset.id.as_str()
            );
        }
    }

    // ── find_preset_by_str edge cases ─────────────────────────────────────────

    #[test]
    fn find_preset_by_str_empty_returns_none() {
        assert!(
            find_preset_by_str("").is_none(),
            "empty string must return None — no preset has an empty ID"
        );
    }

    #[test]
    fn find_preset_by_str_whitespace_returns_none() {
        assert!(
            find_preset_by_str("   ").is_none(),
            "whitespace-only string must return None"
        );
    }

    #[test]
    fn find_preset_by_str_case_insensitive_matches() {
        // The function documents case-insensitive lookup; all casing variants
        // of a valid ID must resolve to the same preset.
        let lower = find_preset_by_str("ubuntu-server-lts");
        let upper = find_preset_by_str("UBUNTU-SERVER-LTS");
        let mixed = find_preset_by_str("Ubuntu-Server-LTS");
        assert!(lower.is_some(), "lowercase must match");
        assert!(upper.is_some(), "uppercase must match");
        assert!(mixed.is_some(), "mixed-case must match");
        assert_eq!(lower.unwrap().id, upper.unwrap().id);
        assert_eq!(lower.unwrap().id, mixed.unwrap().id);
    }

    #[test]
    fn find_preset_by_str_partial_id_returns_none() {
        // "ubuntu" alone must not match "ubuntu-server-lts".
        assert!(
            find_preset_by_str("ubuntu").is_none(),
            "partial ID must not match — exact equality required"
        );
    }

    // ── Catalog invariants ────────────────────────────────────────────────────

    #[test]
    fn all_presets_have_non_empty_names_and_distros() {
        for preset in all_presets() {
            assert!(
                !preset.name.is_empty(),
                "preset {} has empty name",
                preset.id.as_str()
            );
            assert!(
                !preset.distro.is_empty(),
                "preset {} has empty distro",
                preset.id.as_str()
            );
            assert!(
                !preset.edition.is_empty(),
                "preset {} has empty edition",
                preset.id.as_str()
            );
        }
    }

    #[test]
    fn all_direct_url_presets_have_direct_url_set() {
        for preset in all_presets() {
            if preset.strategy == AcquisitionStrategy::DirectUrl {
                assert!(
                    preset.direct_url.is_some(),
                    "DirectUrl preset {} must have direct_url set",
                    preset.id.as_str()
                );
            }
        }
    }

    #[test]
    fn discovery_page_and_user_provided_presets_have_no_direct_url() {
        for preset in all_presets() {
            if matches!(
                preset.strategy,
                AcquisitionStrategy::DiscoveryPage | AcquisitionStrategy::UserProvided
            ) {
                assert!(
                    preset.direct_url.is_none(),
                    "Non-DirectUrl preset {} must not have direct_url set",
                    preset.id.as_str()
                );
            }
        }
    }

    #[test]
    fn all_preset_ids_are_unique() {
        let ids: Vec<&str> = all_presets().iter().map(|p| p.id.as_str()).collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(*id), "duplicate preset id: {id}");
        }
    }

    #[test]
    fn all_presets_have_official_page_starting_with_https() {
        for preset in all_presets() {
            assert!(
                preset.official_page.starts_with("https://"),
                "preset {} official_page must use HTTPS, got: {}",
                preset.id.as_str(),
                preset.official_page
            );
        }
    }

    #[test]
    fn resolve_url_user_provided_always_returns_none() {
        // rhel-custom is UserProvided — user must supply their own ISO path.
        let p = find_preset_by_str("rhel-custom").expect("rhel-custom must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::UserProvided);
        assert!(
            resolve_url(p).unwrap().is_none(),
            "UserProvided must always resolve to None"
        );
    }

    #[test]
    fn resolve_url_direct_url_returns_https_url() {
        // ubuntu-server-lts is DirectUrl — resolve_url must return the CDN URL.
        let p = find_preset_by_str("ubuntu-server-lts").expect("ubuntu-server-lts must exist");
        assert_eq!(p.strategy, AcquisitionStrategy::DirectUrl);
        let url = resolve_url(p)
            .expect("resolve must not error")
            .expect("must return Some URL");
        assert!(
            url.starts_with("https://"),
            "resolved URL must be HTTPS, got: {url}"
        );
    }
}
