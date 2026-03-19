use forgeiso_engine::ForgeIsoEngine;

pub async fn handle(
    engine: &ForgeIsoEngine,
    source: String,
    sums_url: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let result = engine.verify(&source, sums_url.as_deref()).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Verifying: {}", result.filename);
        println!("Expected: {}", result.expected);
        println!("Actual:   {}", result.actual);
        println!("Match:    {}", result.matched);
    }
    if !result.matched {
        std::process::exit(1);
    }
    Ok(())
}
