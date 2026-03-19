use forgeiso_engine::{all_presets, ForgeIsoEngine};

pub async fn handle(engine: &ForgeIsoEngine, json: bool) -> anyhow::Result<()> {
    let report = engine.doctor().await;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("ForgeISO doctor @ {}", report.timestamp);
        println!("Host: {} {}", report.host_os, report.host_arch);
        println!("Linux build support: {}", report.linux_supported);
        println!("Tooling:");
        for (name, available) in &report.tooling {
            let marker = if *available { "ok" } else { "MISSING" };
            println!("  [{marker}] {name}");
        }
        println!("Distro readiness:");
        for (distro, ready) in &report.distro_readiness {
            let marker = if *ready { "ready" } else { "not ready" };
            println!("  [{marker}] {distro}");
        }
        for warning in &report.warnings {
            println!("warning: {warning}");
        }
        println!("Source presets:");
        println!("  {} built-in presets available", all_presets().len());
        println!("  Run 'forgeiso sources list' to see all");
    }
    Ok(())
}
