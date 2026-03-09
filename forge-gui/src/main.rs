mod app;
mod state;
mod worker;

fn main() -> eframe::Result {
    // Capture panics to a log file so crashes are diagnosable even when the
    // app is launched without a terminal (e.g. from the desktop icon).
    let log_path = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".cache/forgeiso/crash.log"))
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/forgeiso-crash.log"));
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let log_path_clone = log_path.clone();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::capture();
        let msg = format!("=== ForgeISO crash ===\n{info}\n\nBacktrace:\n{bt}\n");
        eprintln!("{msg}");
        let _ = std::fs::write(&log_path_clone, &msg);
    }));

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "forge_gui=info,forgeiso_engine=info",
        ))
        .init();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1100.0, 720.0])
            .with_title("ForgeISO — ISO Pipeline Wizard"),
        ..Default::default()
    };

    eframe::run_native(
        "ForgeISO",
        opts,
        Box::new(move |cc| Ok(Box::new(app::ForgeApp::new(cc, rt)))),
    )
}
