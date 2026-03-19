use std::path::PathBuf;

use forgeiso_engine::ForgeIsoEngine;

pub async fn handle(
    engine: &ForgeIsoEngine,
    base: PathBuf,
    target: PathBuf,
    json: bool,
) -> anyhow::Result<()> {
    let result = engine.diff_isos(&base, &target).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("ISO Diff: {} vs {}", base.display(), target.display());
        println!();
        if !result.added.is_empty() {
            println!("Added ({}):", result.added.len());
            for file in &result.added {
                println!("  + {}", file);
            }
            println!();
        }
        if !result.removed.is_empty() {
            println!("Removed ({}):", result.removed.len());
            for file in &result.removed {
                println!("  - {}", file);
            }
            println!();
        }
        if !result.modified.is_empty() {
            println!("Modified ({}):", result.modified.len());
            for entry in &result.modified {
                println!(
                    "  ~ {} ({} \u{2192} {})",
                    entry.path, entry.base_size, entry.target_size
                );
            }
            println!();
        }
        println!("Unchanged: {}", result.unchanged);
    }
    Ok(())
}
