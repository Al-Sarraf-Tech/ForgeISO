use std::path::PathBuf;

use forgeiso_engine::ForgeIsoEngine;

pub async fn handle(engine: &ForgeIsoEngine, build: PathBuf, format: String) -> anyhow::Result<()> {
    let path = engine.report(&build, &format).await?;
    println!("{}", path.display());
    Ok(())
}
