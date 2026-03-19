use std::path::PathBuf;

use forgeiso_engine::ForgeIsoEngine;

pub async fn handle(
    engine: &ForgeIsoEngine,
    artifact: PathBuf,
    policy: Option<PathBuf>,
    json: bool,
) -> anyhow::Result<()> {
    let out = artifact
        .parent()
        .map(|p| p.join("scan"))
        .unwrap_or_else(|| PathBuf::from("scan"));
    let result = engine.scan(&artifact, policy.as_deref(), &out).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("scan report: {}", result.report_json.display());
        for report in result.report.reports {
            println!(
                "  - {}: {:?} ({})",
                report.tool, report.status, report.message
            );
        }
    }
    Ok(())
}
