use forgeiso_engine::ForgeIsoEngine;

pub async fn handle(engine: &ForgeIsoEngine, source: String, json: bool) -> anyhow::Result<()> {
    let info = engine.inspect_source(&source, None).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("Source: {}", info.source_value);
        println!("Cached path: {}", info.source_path.display());
        println!(
            "Detected: distro={} release={} arch={}",
            info.distro
                .map(|value| format!("{:?}", value))
                .unwrap_or_else(|| "unknown".to_string()),
            info.release.as_deref().unwrap_or("unknown"),
            info.architecture.as_deref().unwrap_or("unknown")
        );
        println!(
            "Volume ID: {}",
            info.volume_id.as_deref().unwrap_or("unknown")
        );
        if !info.warnings.is_empty() {
            println!("Warnings:");
            for warning in info.warnings {
                println!("  - {warning}");
            }
        }
    }
    Ok(())
}
