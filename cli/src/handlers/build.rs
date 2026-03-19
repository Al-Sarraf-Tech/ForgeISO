use std::path::PathBuf;

use forgeiso_engine::{BuildConfig, ForgeIsoEngine, IsoSource};

use crate::{parse_profile, resolve_source_from_preset_or_str};

#[allow(clippy::too_many_arguments)]
pub async fn handle(
    engine: &ForgeIsoEngine,
    source: Option<String>,
    preset: Option<String>,
    project: Option<PathBuf>,
    out: PathBuf,
    name: Option<String>,
    overlay: Option<PathBuf>,
    volume_label: Option<String>,
    profile: Option<String>,
    expected_sha256: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let cfg = if let Some(project) = project {
        BuildConfig::from_path(&project)?
    } else {
        // Resolve source: --preset takes precedence over --source when both absent
        let (resolved_source, _preset_distro_tag) =
            resolve_source_from_preset_or_str(source, preset)?;
        BuildConfig {
            name: name.unwrap_or_else(|| "forgeiso-build".to_string()),
            source: IsoSource::from_raw(resolved_source),
            overlay_dir: overlay,
            output_label: volume_label,
            profile: parse_profile(profile.as_deref().unwrap_or("minimal"))?,
            auto_scan: false,
            auto_test: false,
            scanning: Default::default(),
            testing: Default::default(),
            keep_workdir: false,
            expected_sha256,
        }
    };

    let result = engine.build(&cfg, &out).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        if let Some(iso) = result.artifacts.first() {
            println!("Built ISO: {}", iso.display());
        }
        println!("Report JSON: {}", result.report_json.display());
        println!("Report HTML: {}", result.report_html.display());
        println!(
            "Detected source: distro={} release={}",
            result
                .iso
                .distro
                .map(|value| format!("{:?}", value))
                .unwrap_or_else(|| "unknown".to_string()),
            result.iso.release.as_deref().unwrap_or("unknown")
        );
    }
    Ok(())
}
