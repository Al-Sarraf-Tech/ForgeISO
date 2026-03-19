use forgeiso_engine::sources::format_preset_detail;
use forgeiso_engine::{all_presets, find_preset_by_str, resolve_url, AcquisitionStrategy};

use crate::SourcesCmd;

pub async fn handle(command: SourcesCmd) -> anyhow::Result<()> {
    match command {
        SourcesCmd::List { json } => {
            let presets = all_presets();
            if json {
                println!("{}", serde_json::to_string_pretty(presets)?);
            } else {
                println!("{:<28} {:<12} {:<16} NOTE", "PRESET", "DISTRO", "STRATEGY");
                println!("{}", "-".repeat(90));
                for p in presets {
                    println!(
                        "{:<28} {:<12} {:<16} {}",
                        p.id.as_str(),
                        p.distro,
                        p.strategy.as_str(),
                        &p.note.chars().take(50).collect::<String>()
                    );
                }
                println!(
                    "\n{} presets. Run 'forgeiso sources show <PRESET>' for details.",
                    presets.len()
                );
            }
        }
        SourcesCmd::Show { preset, json } => {
            let p = find_preset_by_str(&preset).ok_or_else(|| {
                let ids: Vec<_> = all_presets().iter().map(|p| p.id.as_str()).collect();
                anyhow::anyhow!("unknown preset '{}'. Available: {}", preset, ids.join(", "))
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(p)?);
            } else {
                println!("{}", format_preset_detail(p));
            }
        }
        SourcesCmd::Resolve { preset, json } => {
            let p = find_preset_by_str(&preset).ok_or_else(|| {
                let ids: Vec<_> = all_presets().iter().map(|p| p.id.as_str()).collect();
                anyhow::anyhow!("unknown preset '{}'. Available: {}", preset, ids.join(", "))
            })?;
            let url = resolve_url(p)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "preset": p.id.as_str(),
                        "strategy": p.strategy.as_str(),
                        "url": url,
                        "official_page": p.official_page,
                        "checksum_url": p.checksum_url,
                        "note": p.note,
                    }))?
                );
            } else {
                match &url {
                    Some(u) => {
                        println!("Preset:        {}", p.id.as_str());
                        println!("URL:           {u}");
                        if let Some(c) = p.checksum_url {
                            println!("Checksums:     {c}");
                        }
                    }
                    None => match p.strategy {
                        AcquisitionStrategy::DiscoveryPage => {
                            println!("Preset:        {}", p.id.as_str());
                            println!("Strategy:      discovery-page (URL changes each release)");
                            println!("Official page: {}", p.official_page);
                            println!("Note:          {}", p.note);
                            println!("\nVisit the official page to find the current download URL,");
                            println!("then use: forgeiso inject --source <URL> ...");
                        }
                        AcquisitionStrategy::UserProvided => {
                            println!("Preset:        {}", p.id.as_str());
                            println!("Strategy:      user-provided (BYO ISO)");
                            println!("Official page: {}", p.official_page);
                            println!("Note:          {}", p.note);
                            println!("\nProvide your own ISO path: forgeiso inject --source /path/to/rhel.iso ...");
                        }
                        AcquisitionStrategy::DirectUrl => {
                            eprintln!(
                                "error: preset '{}' is DirectUrl but has no URL configured",
                                p.id.as_str()
                            );
                        }
                    },
                }
            }
        }
    }
    Ok(())
}
