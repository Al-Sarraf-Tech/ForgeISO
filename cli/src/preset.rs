//! Helpers for translating CLI preset/source flags and string profile names
//! into engine types.

use forgeiso_engine::{
    all_presets, find_preset_by_str, resolve_url, AcquisitionStrategy, ProfileKind,
};

pub(crate) fn parse_profile(raw: &str) -> anyhow::Result<ProfileKind> {
    match raw {
        "minimal" => Ok(ProfileKind::Minimal),
        "desktop" => Ok(ProfileKind::Desktop),
        other => anyhow::bail!("unsupported profile '{other}': expected minimal|desktop"),
    }
}

/// Resolve a source URL/path from either --preset or --source flags.
/// Returns an error if neither is provided or if the preset strategy requires user input.
/// Returns (source_url_or_path, preset_distro_tag).
/// `preset_distro_tag` is `Some("fedora")`, `Some("mint")`, etc. when a preset was
/// matched and `None` when the source was provided directly (user controls --distro).
pub(crate) fn resolve_source_from_preset_or_str(
    source: Option<String>,
    preset: Option<String>,
) -> anyhow::Result<(String, Option<&'static str>)> {
    if let Some(preset_name) = preset {
        let ids: Vec<&str> = all_presets().iter().map(|p| p.id.as_str()).collect();
        let found = find_preset_by_str(&preset_name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown preset '{}'. Available: {}",
                preset_name,
                ids.join(", ")
            )
        })?;
        match found.strategy {
            AcquisitionStrategy::DirectUrl => {
                let url = resolve_url(found)?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "preset '{}' is DirectUrl but has no direct_url configured",
                        found.id.as_str()
                    )
                })?;
                Ok((url, Some(found.distro)))
            }
            AcquisitionStrategy::DiscoveryPage => {
                anyhow::bail!(
                    "preset '{}' uses a discovery page \u{2014} visit {} to find the current ISO URL, \
                     then use --source <URL>",
                    found.id.as_str(),
                    found.official_page
                );
            }
            AcquisitionStrategy::UserProvided => {
                anyhow::bail!(
                    "preset '{}' requires you to supply your own ISO \u{2014} visit {} and use --source <path>",
                    found.id.as_str(),
                    found.official_page
                );
            }
        }
    } else if let Some(s) = source {
        Ok((s, None))
    } else {
        anyhow::bail!("--source or --preset is required")
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_profile, resolve_source_from_preset_or_str};

    #[test]
    fn parse_profile_accepts_supported_profiles() {
        assert!(parse_profile("minimal").is_ok());
        assert!(parse_profile("desktop").is_ok());
        assert!(parse_profile("broken").is_err());
    }

    #[test]
    fn resolve_source_requires_source_or_preset() {
        let err =
            resolve_source_from_preset_or_str(None, None).expect_err("missing source must fail");
        assert!(err.to_string().contains("--source or --preset is required"));
    }
}
