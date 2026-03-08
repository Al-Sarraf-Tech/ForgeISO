mod app;
mod state;
mod worker;

fn main() -> eframe::Result {
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
