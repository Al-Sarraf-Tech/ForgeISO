use std::path::PathBuf;

use forgeiso_engine::ForgeIsoEngine;

pub async fn handle(
    engine: &ForgeIsoEngine,
    iso: PathBuf,
    bios: bool,
    uefi: bool,
    json: bool,
) -> anyhow::Result<()> {
    let run_bios = bios || !uefi;
    let run_uefi = uefi || !bios;
    let out = iso
        .parent()
        .map(|p| p.join("test"))
        .unwrap_or_else(|| PathBuf::from("test"));
    let result = engine.test_iso(&iso, run_bios, run_uefi, &out).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "bios={} uefi={} passed={}",
            result.bios, result.uefi, result.passed
        );
        for log in result.logs {
            println!("  - {}", log.display());
        }
    }
    Ok(())
}
