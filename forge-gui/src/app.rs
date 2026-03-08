use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use egui::{Color32, Frame, RichText, Stroke, Ui, Vec2};
use forgeiso_engine::{
    BuildConfig, ContainerConfig, Distro, FirewallConfig, ForgeIsoEngine, GrubConfig, InjectConfig,
    IsoSource, NetworkConfig, ProfileKind, ProxyConfig, SshConfig, SwapConfig, UserConfig,
};
use serde::{Deserialize, Serialize};

use crate::state::{
    lines, opt, BuildResult, BuildState, DiffFilter, DiffState, DoctorReport, InjectState,
    Iso9660Compliance, IsoDiff, IsoMetadata, LogEntry, LogLevel, PickTarget, StatusMsg,
    VerifyResult, VerifyState,
};
use crate::worker::{self, WorkerMsg};

// ── Persisted form state ───────────────────────────────────────────────────────

const STORAGE_KEY: &str = "forgeiso_v2";

#[derive(Default, Serialize, Deserialize)]
struct PersistedState {
    inject: InjectState,
    verify: VerifyState,
    diff: DiffState,
    build: BuildState,
}

// ── Palette ───────────────────────────────────────────────────────────────────

const BG: Color32 = Color32::from_rgb(13, 17, 23);
const SURFACE: Color32 = Color32::from_rgb(22, 27, 34);
const BORDER: Color32 = Color32::from_rgb(48, 54, 61);
const ACCENT: Color32 = Color32::from_rgb(47, 129, 247);
const GREEN: Color32 = Color32::from_rgb(63, 185, 80);
const RED: Color32 = Color32::from_rgb(248, 81, 73);
const AMBER: Color32 = Color32::from_rgb(210, 153, 34);
const TEXT: Color32 = Color32::from_rgb(230, 237, 243);
const MUTED: Color32 = Color32::from_rgb(139, 148, 158);
const TAB_ACTIVE: Color32 = Color32::from_rgb(33, 38, 45);
// ── UI helpers ────────────────────────────────────────────────────────────────

/// Thin muted label that sits above a field.
fn lbl(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(14.0).color(MUTED));
}

/// Section title inside a form.
fn section(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).size(14.0).strong().color(TEXT));
    ui.add_space(4.0);
}

/// Thin horizontal rule.
fn rule(ui: &mut Ui) {
    ui.add_space(14.0);
    ui.add(egui::Separator::default().horizontal().spacing(0.0));
    ui.add_space(14.0);
}

/// Full-width primary action button.
fn action_btn(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let fill = if enabled {
        ACCENT
    } else {
        Color32::from_rgb(33, 38, 45)
    };
    let text_col = if enabled { Color32::WHITE } else { MUTED };
    let btn = egui::Button::new(RichText::new(label).size(16.0).strong().color(text_col))
        .fill(fill)
        .stroke(Stroke::new(1.0, if enabled { ACCENT } else { BORDER }))
        .min_size(Vec2::new(ui.available_width(), 52.0));
    ui.add_enabled(enabled, btn).clicked()
}

/// Small "Browse" button that fits next to a text field.
fn browse_btn(ui: &mut Ui, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new("Browse").size(13.0))
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .min_size(Vec2::new(80.0, 38.0)),
    )
    .clicked()
}

/// Small secondary button.
fn small_btn(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(13.0))
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .min_size(Vec2::new(100.0, 38.0)),
    )
    .clicked()
}

/// Green "Continue" button.
fn continue_btn(ui: &mut Ui, label: &str) -> bool {
    let btn = egui::Button::new(
        RichText::new(label)
            .size(14.0)
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(GREEN)
    .stroke(Stroke::new(1.0, GREEN))
    .min_size(Vec2::new(180.0, 42.0));
    ui.add(btn).clicked()
}

/// Coloured result card.
fn result_box(ui: &mut Ui, fill: Color32, border: Color32, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .inner_margin(14.0f32)
        .corner_radius(6.0f32)
        .show(ui, add);
    ui.add_space(8.0);
}

fn card_green(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    result_box(
        ui,
        Color32::from_rgb(13, 28, 18),
        Color32::from_rgb(40, 100, 55),
        add,
    );
}

fn now_ts() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn fmt_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1_048_576 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else if n < 1_073_741_824 {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", n as f64 / 1_073_741_824.0)
    }
}

fn distro_label(d: &forgeiso_engine::Distro) -> String {
    use forgeiso_engine::Distro;
    match d {
        Distro::Ubuntu => "Ubuntu".into(),
        Distro::Fedora => "Fedora".into(),
        Distro::Arch => "Arch Linux".into(),
        Distro::Mint => "Linux Mint".into(),
    }
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct ForgeApp {
    rt: tokio::runtime::Runtime,
    engine: Arc<ForgeIsoEngine>,
    tx: mpsc::Sender<WorkerMsg>,
    rx: mpsc::Receiver<WorkerMsg>,
    // Navigation — now tab-based, no sidebar
    active_tab: Tab,
    // Job
    job_running: bool,
    job_phase: String,
    job_pct: Option<f32>,
    current_task: Option<tokio::task::JoinHandle<()>>,
    // Forms
    inject: InjectState,
    verify: VerifyState,
    diff: DiffState,
    build: BuildState,
    // Results
    inject_result: Option<BuildResult>,
    verify_result: Option<VerifyResult>,
    iso9660_result: Option<Iso9660Compliance>,
    diff_result: Option<IsoDiff>,
    build_result: Option<BuildResult>,
    inspect_result: Option<IsoMetadata>,
    // Done flags
    inject_done: bool,
    verify_done: bool,
    diff_done: bool,
    build_done: bool,
    // Log
    log_entries: Vec<LogEntry>,
    log_open: bool,
    log_errors_only: bool,
    // Status
    status: Option<StatusMsg>,
    status_since: Option<std::time::Instant>,
    // Diff filter
    diff_filter: DiffFilter,
    diff_search: String,
    // Doctor
    doctor_result: Option<DoctorReport>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Inject,
    Verify,
    Diff,
    Build,
    Doctor,
}

impl Tab {
    fn label(&self) -> &'static str {
        match self {
            Tab::Inject => "Inject",
            Tab::Verify => "Verify",
            Tab::Diff => "Diff",
            Tab::Build => "Build",
            Tab::Doctor => "Doctor",
        }
    }
}

impl ForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Runtime) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = BG;
        visuals.panel_fill = BG;
        visuals.widgets.noninteractive.bg_fill = SURFACE;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(27, 32, 39);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(33, 38, 45);
        visuals.widgets.active.bg_fill = ACCENT;
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(47, 129, 247, 70);
        visuals.override_text_color = Some(TEXT);
        visuals.window_stroke = Stroke::new(1.0, BORDER);
        cc.egui_ctx.set_visuals(visuals);

        let mut style = (*cc.egui_ctx.style()).clone();
        use egui::{FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(18.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Small, FontId::proportional(11.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        cc.egui_ctx.set_style(style);

        let (tx, rx) = mpsc::channel();
        let engine = Arc::new(ForgeIsoEngine::new());

        let persisted: PersistedState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        {
            let mut ev_rx = engine.subscribe();
            let tx2 = tx.clone();
            rt.spawn(async move {
                while let Ok(ev) = ev_rx.recv().await {
                    use forgeiso_engine::EventLevel;
                    let is_error = matches!(ev.level, EventLevel::Error);
                    let is_warn = matches!(ev.level, EventLevel::Warn);
                    let _ = tx2.send(WorkerMsg::EngineEvent {
                        phase: format!("{:?}", ev.phase),
                        message: ev.message.clone(),
                        percent: ev.percent.map(|p| p / 100.0),
                        is_error,
                        is_warn,
                    });
                }
            });
        }

        Self {
            rt,
            engine,
            tx,
            rx,
            active_tab: Tab::Inject,
            job_running: false,
            job_phase: String::new(),
            job_pct: None,
            current_task: None,
            inject: persisted.inject,
            verify: persisted.verify,
            diff: persisted.diff,
            build: persisted.build,
            inject_result: None,
            verify_result: None,
            iso9660_result: None,
            diff_result: None,
            build_result: None,
            inspect_result: None,
            inject_done: false,
            verify_done: false,
            diff_done: false,
            build_done: false,
            log_entries: Vec::new(),
            log_open: false,
            log_errors_only: false,
            status: None,
            status_since: None,
            diff_filter: DiffFilter::All,
            diff_search: String::new(),
            doctor_result: None,
        }
    }

    // ── Message handling ───────────────────────────────────────────────────────

    fn drain_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle_msg(msg);
            ctx.request_repaint();
        }
        if let Some(t) = self.status_since {
            if t.elapsed().as_secs() >= 8
                && self.status.as_ref().map(|s| !s.is_error).unwrap_or(false)
            {
                self.status = None;
                self.status_since = None;
            }
        }
    }

    fn handle_msg(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::EngineEvent {
                phase,
                message,
                percent,
                is_error,
                is_warn,
            } => {
                self.job_phase = phase.clone();
                self.job_pct = percent;
                self.log_entries.push(LogEntry {
                    phase,
                    message,
                    level: if is_error {
                        LogLevel::Error
                    } else if is_warn {
                        LogLevel::Warn
                    } else {
                        LogLevel::Info
                    },
                    timestamp: now_ts(),
                });
            }
            WorkerMsg::InjectOk(r) => {
                self.inject_done = true;
                let src = r.source_iso.to_string_lossy().into_owned();
                if let Some(path) = r.artifacts.first() {
                    let out = path.to_string_lossy().into_owned();
                    if self.verify.source.is_empty() {
                        self.verify.source = src.clone();
                    }
                    if self.diff.base.is_empty() {
                        self.diff.base = src;
                    }
                    if self.diff.target.is_empty() {
                        self.diff.target = out.clone();
                    }
                    if self.build.source.is_empty() {
                        self.build.source = out;
                    }
                }
                self.inject_result = Some(*r);
                self.job_running = false;
                self.set_status(StatusMsg::ok("Inject complete"));
            }
            WorkerMsg::VerifyOk(r) => {
                let matched = r.matched;
                self.verify_result = Some(*r);
                self.verify_done = true;
                self.job_running = false;
                self.set_status(if matched {
                    StatusMsg::ok("Checksum matched")
                } else {
                    StatusMsg::err("Checksum mismatch")
                });
            }
            WorkerMsg::Iso9660Ok(r) => {
                let ok = r.compliant;
                self.iso9660_result = Some(*r);
                self.job_running = false;
                self.set_status(if ok {
                    StatusMsg::ok("ISO-9660 compliant")
                } else {
                    StatusMsg::err("ISO-9660 non-compliant")
                });
            }
            WorkerMsg::DiffOk(r) => {
                let total = r.added.len() + r.removed.len() + r.modified.len();
                self.diff_result = Some(*r);
                self.diff_done = true;
                self.job_running = false;
                self.set_status(StatusMsg::ok(format!("Diff: {total} changed files")));
            }
            WorkerMsg::BuildOk(r) => {
                let path = r
                    .artifacts
                    .first()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.build_result = Some(*r);
                self.build_done = true;
                self.job_running = false;
                self.set_status(StatusMsg::ok(format!("Build complete: {path}")));
            }
            WorkerMsg::InspectOk(m) => {
                self.inspect_result = Some(*m);
                self.job_running = false;
                self.set_status(StatusMsg::ok("Inspection complete"));
            }
            WorkerMsg::DoctorOk(r) => {
                self.doctor_result = Some(*r);
                self.job_running = false;
                self.set_status(StatusMsg::ok("Doctor check complete"));
            }
            WorkerMsg::ScanOk => {
                self.job_running = false;
                self.set_status(StatusMsg::ok("Scan complete"));
            }
            WorkerMsg::TestOk => {
                self.job_running = false;
                self.set_status(StatusMsg::ok("Boot test complete"));
            }
            WorkerMsg::ReportOk(path) => {
                self.job_running = false;
                self.set_status(StatusMsg::ok(format!("Report: {path}")));
            }
            WorkerMsg::FilePicked { target, path } => match target {
                PickTarget::InjectSource => self.inject.source = path,
                PickTarget::InjectOutputDir => self.inject.output_dir = path,
                PickTarget::InjectWallpaper => self.inject.wallpaper_path = path,
                PickTarget::VerifySource => self.verify.source = path,
                PickTarget::DiffBase => self.diff.base = path,
                PickTarget::DiffTarget => self.diff.target = path,
                PickTarget::BuildSource => self.build.source = path,
                PickTarget::BuildOutputDir => self.build.output_dir = path,
                PickTarget::BuildOverlay => self.build.overlay_dir = path,
            },
            WorkerMsg::OpError(e) => {
                self.job_running = false;
                self.log_entries.push(LogEntry {
                    phase: "Error".into(),
                    message: e.clone(),
                    level: LogLevel::Error,
                    timestamp: now_ts(),
                });
                self.set_status(StatusMsg::err(e));
            }
            WorkerMsg::Done => {
                self.job_running = false;
            }
        }
    }

    // ── Engine spawn helpers ───────────────────────────────────────────────────

    fn start_job(&mut self, phase: &str) {
        self.job_running = true;
        self.job_phase = phase.into();
        self.job_pct = None;
        self.status = None;
        self.status_since = None;
    }

    fn set_status(&mut self, msg: StatusMsg) {
        self.status_since = Some(std::time::Instant::now());
        self.status = Some(msg);
    }

    fn cancel_job(&mut self) {
        if let Some(handle) = self.current_task.take() {
            handle.abort();
        }
        self.job_running = false;
        self.job_pct = None;
        self.set_status(StatusMsg::ok("Cancelled"));
    }

    fn spawn_inject(&mut self) {
        self.inject_done = false;
        self.inject_result = None;
        self.start_job("Injecting…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let inject = self.inject.clone();
        let out = PathBuf::from(&inject.output_dir);
        self.current_task = Some(self.rt.spawn(async move {
            let cfg = build_inject_config(&inject);
            match engine.inject_autoinstall(&cfg, &out).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::InjectOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_verify(&mut self) {
        self.verify_done = false;
        self.verify_result = None;
        self.iso9660_result = None;
        self.start_job("Verifying checksum…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let source = self.verify.source.clone();
        let sums = opt(&self.verify.sums_url);
        self.current_task = Some(self.rt.spawn(async move {
            match engine.verify(&source, sums.as_deref()).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::VerifyOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_iso9660(&mut self) {
        self.iso9660_result = None;
        self.start_job("Validating ISO-9660…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let source = self.verify.source.clone();
        self.current_task = Some(self.rt.spawn(async move {
            match engine.validate_iso9660(&source).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::Iso9660Ok(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_diff(&mut self) {
        self.diff_done = false;
        self.diff_result = None;
        self.start_job("Comparing ISOs…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let base = PathBuf::from(&self.diff.base);
        let target = PathBuf::from(&self.diff.target);
        self.current_task = Some(self.rt.spawn(async move {
            match engine.diff_isos(&base, &target).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::DiffOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_build(&mut self) {
        self.build_done = false;
        self.build_result = None;
        self.start_job("Building ISO…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let b = self.build.clone();
        let out = PathBuf::from(&b.output_dir);
        self.current_task = Some(self.rt.spawn(async move {
            let cfg = BuildConfig {
                name: b.build_name.clone(),
                source: IsoSource::from_raw(&b.source),
                overlay_dir: opt(&b.overlay_dir).map(PathBuf::from),
                output_label: opt(&b.output_label),
                profile: if b.profile == "desktop" {
                    ProfileKind::Desktop
                } else {
                    ProfileKind::Minimal
                },
                auto_scan: false,
                auto_test: false,
                scanning: Default::default(),
                testing: Default::default(),
                keep_workdir: false,
                expected_sha256: opt(&b.expected_sha256),
            };
            match engine.build(&cfg, &out).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::BuildOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_inspect(&mut self) {
        self.start_job("Inspecting ISO…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let source = self.build.source.clone();
        self.current_task = Some(self.rt.spawn(async move {
            match engine.inspect_source(&source, None).await {
                Ok(m) => {
                    let _ = tx.send(WorkerMsg::InspectOk(Box::new(m)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_doctor(&mut self) {
        self.start_job("Checking dependencies…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        self.current_task = Some(self.rt.spawn(async move {
            let r = engine.doctor().await;
            let _ = tx.send(WorkerMsg::DoctorOk(Box::new(r)));
        }));
    }

    fn spawn_scan(&mut self) {
        let Some(iso) = self
            .build_result
            .as_ref()
            .and_then(|r| r.artifacts.first().cloned())
        else {
            self.set_status(StatusMsg::err("No build artifact to scan"));
            return;
        };
        self.start_job("Scanning artifact…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let out = iso
            .parent()
            .map(|p| p.join("scan"))
            .unwrap_or_else(|| PathBuf::from("scan"));
        self.current_task = Some(self.rt.spawn(async move {
            match engine.scan(&iso, None, &out).await {
                Ok(_) => {
                    let _ = tx.send(WorkerMsg::ScanOk);
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_test_iso(&mut self) {
        let Some(iso) = self
            .build_result
            .as_ref()
            .and_then(|r| r.artifacts.first().cloned())
        else {
            self.set_status(StatusMsg::err("No build artifact to test"));
            return;
        };
        self.start_job("Running boot test…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let out = iso
            .parent()
            .map(|p| p.join("test"))
            .unwrap_or_else(|| PathBuf::from("test"));
        self.current_task = Some(self.rt.spawn(async move {
            match engine.test_iso(&iso, true, true, &out).await {
                Ok(_) => {
                    let _ = tx.send(WorkerMsg::TestOk);
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_report(&mut self, format: &str) {
        let Some(build_dir) = self.build_result.as_ref().map(|r| r.output_dir.clone()) else {
            self.set_status(StatusMsg::err("No build result to report on"));
            return;
        };
        self.start_job(&format!("Rendering {format} report…"));
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let fmt = format.to_string();
        self.current_task = Some(self.rt.spawn(async move {
            match engine.report(&build_dir, &fmt).await {
                Ok(p) => {
                    let _ = tx.send(WorkerMsg::ReportOk(p.to_string_lossy().into_owned()));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    /// Top header bar: logo + status + cancel button.
    fn render_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(10, 14, 20))
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Logo
                    ui.label(
                        RichText::new("ForgeISO")
                            .size(18.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("ISO Customization Platform")
                            .size(13.0)
                            .color(MUTED),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Cancel button if job running
                        if self.job_running {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Cancel").size(14.0).color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(100, 30, 30))
                                    .stroke(Stroke::new(1.0, RED))
                                    .min_size(Vec2::new(72.0, 32.0)),
                                )
                                .clicked()
                            {
                                self.cancel_job();
                            }
                            ui.add_space(8.0);
                            ui.spinner();
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.job_phase.clone())
                                    .size(14.0)
                                    .color(MUTED),
                            );
                            if let Some(pct) = self.job_pct {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(format!("{:.0}%", pct * 100.0))
                                        .size(14.0)
                                        .color(ACCENT),
                                );
                            }
                        } else if let Some(s) = self.status.as_ref() {
                            let col = if s.is_error { RED } else { GREEN };
                            ui.label(RichText::new(&s.text).size(14.0).color(col));
                        }
                    });
                });
            });
    }

    /// Horizontal tab strip under the header.
    fn render_tabs(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tabs")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(13, 17, 23))
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    for tab in &[Tab::Inject, Tab::Verify, Tab::Diff, Tab::Build, Tab::Doctor] {
                        let active = *tab == self.active_tab;
                        let label = tab.label();
                        let done = match tab {
                            Tab::Inject => self.inject_done,
                            Tab::Verify => self.verify_done,
                            Tab::Diff => self.diff_done,
                            Tab::Build => self.build_done,
                            Tab::Doctor => self.doctor_result.is_some(),
                        };
                        let text_col = if active {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(139, 148, 158)
                        };
                        let fill = if active {
                            TAB_ACTIVE
                        } else {
                            Color32::TRANSPARENT
                        };
                        let display = if done && !active {
                            format!("{label} ✓")
                        } else {
                            label.to_string()
                        };
                        let btn =
                            egui::Button::new(RichText::new(display).size(14.0).color(text_col))
                                .fill(fill)
                                .stroke(Stroke::new(
                                    if active { 1.0 } else { 0.0 },
                                    Color32::from_rgb(48, 54, 61),
                                ))
                                .min_size(Vec2::new(100.0, 46.0));
                        if ui.add(btn).clicked() {
                            self.active_tab = *tab;
                        }
                        ui.add_space(4.0);
                    }

                    // Log toggle on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        let error_count = self
                            .log_entries
                            .iter()
                            .filter(|e| e.level == LogLevel::Error)
                            .count();
                        let log_label = if error_count > 0 {
                            format!(
                                "Log ({error_count} error{})",
                                if error_count == 1 { "" } else { "s" }
                            )
                        } else {
                            format!("Log ({})", self.log_entries.len())
                        };
                        let log_col = if error_count > 0 { RED } else { MUTED };
                        let log_btn =
                            egui::Button::new(RichText::new(log_label).size(13.0).color(log_col))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(0.0, BORDER));
                        if ui.add(log_btn).clicked() {
                            self.log_open = !self.log_open;
                        }
                    });
                });
            });
    }

    /// Collapsible log strip at the bottom.
    fn render_log(&mut self, ctx: &egui::Context) {
        if !self.log_open {
            return;
        }
        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .min_height(150.0)
            .default_height(220.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(10, 14, 20))
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Log").size(14.0).strong().color(MUTED));
                    ui.add_space(12.0);
                    ui.checkbox(
                        &mut self.log_errors_only,
                        RichText::new("Errors only").size(13.0).color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Clear").size(13.0).color(MUTED))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(0.0, BORDER)),
                            )
                            .clicked()
                        {
                            self.log_entries.clear();
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("×").size(14.0).color(MUTED))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(0.0, BORDER)),
                            )
                            .clicked()
                        {
                            self.log_open = false;
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for entry in &self.log_entries {
                            if self.log_errors_only && entry.level != LogLevel::Error {
                                continue;
                            }
                            let col = match entry.level {
                                LogLevel::Error => RED,
                                LogLevel::Warn => AMBER,
                                LogLevel::Info => MUTED,
                            };
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&entry.timestamp)
                                        .size(13.0)
                                        .monospace()
                                        .color(Color32::from_rgb(70, 80, 95)),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("[{}]", entry.phase))
                                        .size(13.0)
                                        .monospace()
                                        .color(ACCENT),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(&entry.message)
                                        .size(13.0)
                                        .monospace()
                                        .color(col),
                                );
                            });
                        }
                    });
            });
    }

    // ── Tab content ────────────────────────────────────────────────────────────

    fn show_inject(&mut self, ui: &mut Ui) {
        let running = self.job_running;
        let mut do_inject = false;

        egui::ScrollArea::vertical()
            .id_salt("inject_scroll")
            .show(ui, |ui| {
                ui.add_space(12.0);

                // ── Source ISO ──────────────────────────────────────────────
                lbl(ui, "Source ISO  (local path or URL)");
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.source)
                            .hint_text(
                                "/path/to/ubuntu-24.04.iso  or  https://releases.ubuntu.com/…",
                            )
                            .desired_width(ui.available_width() - 96.0)
                            .min_size(Vec2::new(0.0, 42.0)),
                    );
                    if browse_btn(ui, !running) {
                        worker::pick_iso(PickTarget::InjectSource, self.tx.clone());
                    }
                });

                ui.add_space(10.0);

                // ── Output ─────────────────────────────────────────────────
                ui.columns(2, |cols| {
                    cols[0].vertical(|ui| {
                        lbl(ui, "Output Directory");
                        ui.horizontal(|ui| {
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.inject.output_dir)
                                    .desired_width(f32::INFINITY)
                                    .min_size(Vec2::new(0.0, 38.0)),
                            );
                            if ui
                                .add_enabled(
                                    !running,
                                    egui::Button::new("📂")
                                        .fill(SURFACE)
                                        .stroke(Stroke::new(1.0, BORDER))
                                        .min_size(Vec2::new(40.0, 38.0)),
                                )
                                .on_hover_text("Pick output folder")
                                .clicked()
                            {
                                worker::pick_folder(PickTarget::InjectOutputDir, self.tx.clone());
                            }
                        });
                    });
                    cols[1].vertical(|ui| {
                        lbl(ui, "Output Filename");
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.out_name)
                                .hint_text("forgeiso-local.iso")
                                .desired_width(f32::INFINITY)
                                .min_size(Vec2::new(0.0, 38.0)),
                        );
                    });
                });

                rule(ui);

                // ── Identity ───────────────────────────────────────────────
                section(ui, "Identity");
                // Use ui.columns() so each half fills exactly (available-gap)/2 — Grid
                // only sizes to minimum content width and leaves fields visually narrow.
                ui.columns(2, |cols| {
                    cols[0].vertical(|ui| {
                        lbl(ui, "Hostname");
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.hostname)
                                .hint_text("my-server")
                                .desired_width(f32::INFINITY)
                                .min_size(Vec2::new(0.0, 40.0)),
                        );
                    });
                    cols[1].vertical(|ui| {
                        lbl(ui, "Username");
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.username)
                                .hint_text("admin")
                                .desired_width(f32::INFINITY)
                                .min_size(Vec2::new(0.0, 40.0)),
                        );
                    });
                });
                ui.add_space(12.0);
                ui.columns(2, |cols| {
                    cols[0].vertical(|ui| {
                        lbl(ui, "Password");
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.password)
                                .password(true)
                                .desired_width(f32::INFINITY)
                                .min_size(Vec2::new(0.0, 40.0)),
                        );
                    });
                    let mismatch = !self.inject.password.is_empty()
                        && !self.inject.password_confirm.is_empty()
                        && self.inject.password != self.inject.password_confirm;
                    cols[1].vertical(|ui| {
                        lbl(ui, "Confirm Password");
                        let te = egui::TextEdit::singleline(&mut self.inject.password_confirm)
                            .password(true)
                            .desired_width(f32::INFINITY)
                            .min_size(Vec2::new(0.0, 40.0));
                        let resp = ui.add_enabled(!running, te);
                        if mismatch {
                            resp.on_hover_text(RichText::new("Passwords do not match").color(RED));
                            ui.label(
                                RichText::new("Passwords do not match")
                                    .size(13.0)
                                    .color(RED),
                            );
                        }
                    });
                });

                ui.add_space(8.0);

                // ── Distro ─────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    lbl(ui, "Target Distro:");
                    ui.add_space(8.0);
                    egui::ComboBox::from_id_salt("distro_combo")
                        .selected_text(match self.inject.distro.as_str() {
                            "fedora" => "Fedora",
                            "arch" => "Arch Linux",
                            "mint" => "Linux Mint",
                            _ => "Ubuntu / Mint (default)",
                        })
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.inject.distro,
                                "ubuntu".into(),
                                "Ubuntu / Mint (default)",
                            );
                            ui.selectable_value(
                                &mut self.inject.distro,
                                "fedora".into(),
                                "Fedora  (Kickstart)",
                            );
                            ui.selectable_value(
                                &mut self.inject.distro,
                                "arch".into(),
                                "Arch Linux  (archinstall)",
                            );
                            ui.selectable_value(
                                &mut self.inject.distro,
                                "mint".into(),
                                "Linux Mint  (cloud-init)",
                            );
                        });
                });

                rule(ui);

                // ── Advanced Options ───────────────────────────────────────
                egui::CollapsingHeader::new(
                    RichText::new("⚙  Advanced Options").size(15.0).color(MUTED),
                )
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(10.0);
                    self.show_inject_advanced(ui, running);
                });

                rule(ui);

                // ── Validation & Run ───────────────────────────────────────
                let source_empty = self.inject.source.trim().is_empty();
                let out_empty = self.inject.output_dir.trim().is_empty();
                let pw_mismatch = !self.inject.password.is_empty()
                    && !self.inject.password_confirm.is_empty()
                    && self.inject.password != self.inject.password_confirm;
                let sha_invalid = {
                    let s = self.inject.expected_sha256.trim();
                    !s.is_empty() && (s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()))
                };

                if source_empty {
                    ui.label(
                        RichText::new("Source ISO is required to proceed.")
                            .size(14.0)
                            .color(AMBER),
                    );
                    ui.add_space(4.0);
                }
                if out_empty {
                    ui.label(
                        RichText::new("Output directory is required.")
                            .size(14.0)
                            .color(AMBER),
                    );
                    ui.add_space(4.0);
                }
                if pw_mismatch {
                    ui.label(
                        RichText::new("Passwords do not match.")
                            .size(14.0)
                            .color(RED),
                    );
                    ui.add_space(4.0);
                }
                if sha_invalid {
                    ui.label(
                        RichText::new("SHA-256 must be 64 hex characters.")
                            .size(14.0)
                            .color(RED),
                    );
                    ui.add_space(4.0);
                }

                let can = !source_empty && !out_empty && !pw_mismatch && !sha_invalid && !running;
                let btn_label = if running {
                    "⏳  Injecting…"
                } else {
                    "Inject ISO"
                };
                if action_btn(ui, btn_label, can) {
                    do_inject = true;
                }

                // ── Result ────────────────────────────────────────────────
                if let Some(r) = self.inject_result.clone() {
                    ui.add_space(12.0);
                    result_box(
                        ui,
                        Color32::from_rgb(13, 28, 18),
                        Color32::from_rgb(40, 100, 55),
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("✓  Inject Complete")
                                        .size(14.0)
                                        .strong()
                                        .color(GREEN),
                                );
                            });
                            ui.add_space(8.0);
                            for a in &r.artifacts {
                                let path_str = a.to_string_lossy();
                                let avail = ui.available_width();
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(
                                            &mut path_str.as_ref().to_string(),
                                        )
                                        .desired_width(avail - 130.0)
                                        .interactive(false)
                                        .font(egui::FontId::monospace(12.0)),
                                    );
                                    if ui
                                        .add(
                                            egui::Button::new("📋 Copy")
                                                .fill(SURFACE)
                                                .stroke(Stroke::new(1.0, BORDER)),
                                        )
                                        .clicked()
                                    {
                                        ui.ctx().copy_text(path_str.into_owned());
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new("📂 Open")
                                                .fill(SURFACE)
                                                .stroke(Stroke::new(1.0, BORDER)),
                                        )
                                        .clicked()
                                    {
                                        if let Some(dir) = a.parent() {
                                            if std::process::Command::new("xdg-open")
                                                .arg(dir)
                                                .spawn()
                                                .is_err()
                                            {
                                                self.set_status(StatusMsg::err(
                                                    "xdg-open failed — open the folder manually",
                                                ));
                                            }
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if continue_btn(ui, "→  Verify") {
                                    self.active_tab = Tab::Verify;
                                }
                                ui.add_space(8.0);
                                if continue_btn(ui, "→  Diff") {
                                    self.active_tab = Tab::Diff;
                                }
                            });
                        },
                    );
                }

                ui.add_space(16.0);
            });

        if do_inject {
            self.spawn_inject();
        }
    }

    fn show_inject_advanced(&mut self, ui: &mut Ui, running: bool) {
        let full_w = ui.available_width();
        let col_w = (full_w - 20.0) / 2.0;

        // ── SSH ──────────────────────────────────────────────────────────
        section(ui, "SSH");
        lbl(ui, "Authorized Public Keys  (one per line)");
        ui.add_enabled(
            !running,
            egui::TextEdit::multiline(&mut self.inject.ssh_keys)
                .hint_text("ssh-ed25519 AAAA…")
                .desired_width(full_w)
                .desired_rows(4),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_enabled(
                !running,
                egui::Checkbox::new(
                    &mut self.inject.ssh_install_server,
                    "Install OpenSSH server",
                ),
            );
            ui.add_space(16.0);
            ui.add_enabled(
                !running,
                egui::Checkbox::new(&mut self.inject.ssh_password_auth, "Allow password auth"),
            );
        });

        rule(ui);

        // ── Network ───────────────────────────────────────────────────────
        section(ui, "Network");
        egui::Grid::new("adv_net_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "DNS Servers  (one per line)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.dns_servers)
                            .hint_text("8.8.8.8\n1.1.1.1")
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "NTP Servers  (one per line)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.ntp_servers)
                            .hint_text("pool.ntp.org")
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                });
                ui.end_row();
                ui.vertical(|ui| {
                    lbl(ui, "Static IP / CIDR  (blank = DHCP)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.static_ip)
                            .hint_text("192.168.1.10/24")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Gateway");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.gateway)
                            .hint_text("192.168.1.1")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
                ui.vertical(|ui| {
                    lbl(ui, "HTTP Proxy");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.http_proxy)
                            .hint_text("http://proxy:3128")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "HTTPS Proxy");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.https_proxy)
                            .hint_text("http://proxy:3128")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
            });
        lbl(ui, "No-proxy (comma-separated)");
        ui.add_enabled(
            !running,
            egui::TextEdit::singleline(&mut self.inject.no_proxy)
                .hint_text("localhost,127.0.0.1,.internal")
                .desired_width(full_w),
        );

        rule(ui);

        // ── System ────────────────────────────────────────────────────────
        section(ui, "System");
        egui::Grid::new("adv_sys_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "Timezone");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.timezone)
                            .hint_text("America/Chicago")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Locale");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.locale)
                            .hint_text("en_US.UTF-8")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
                ui.vertical(|ui| {
                    lbl(ui, "Keyboard Layout");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.keyboard_layout)
                            .hint_text("us")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Storage Layout");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.storage_layout)
                            .hint_text("lvm  (blank = direct)")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
                ui.vertical(|ui| {
                    lbl(ui, "Wallpaper  (image path)");
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.wallpaper_path)
                                .desired_width(f32::INFINITY),
                        );
                        if ui
                            .add_enabled(
                                !running,
                                egui::Button::new("📂")
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(32.0, 24.0)),
                            )
                            .clicked()
                        {
                            worker::pick_file(PickTarget::InjectWallpaper, self.tx.clone());
                        }
                    });
                });
                ui.end_row();
            });

        rule(ui);

        // ── Packages & Repositories ────────────────────────────────────────
        section(ui, "Packages & Repositories");

        // Row 1: packages + APT repos/mirror
        egui::Grid::new("adv_pkg_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "Extra Packages  (one per line)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.packages)
                            .hint_text(
                                "curl\ngit\nvim\nhtop\nunzip\nrsync\nwget\njq\nnet-tools",
                            )
                            .desired_width(f32::INFINITY)
                            .desired_rows(5),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "APT Mirror  (Ubuntu/Debian — click to preset)");
                    ui.horizontal_wrapped(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 6.0;
                        for (label, url) in [
                            ("official",   "http://archive.ubuntu.com/ubuntu"),
                            ("US",         "http://us.archive.ubuntu.com/ubuntu"),
                            ("kernel.org", "http://mirrors.edge.kernel.org/ubuntu"),
                            ("MIT",        "http://mirrors.mit.edu/ubuntu"),
                        ] {
                            if ui.add_enabled(
                                !running,
                                egui::Button::new(label)
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(0.0, 28.0)),
                            ).on_hover_text(url).clicked() {
                                self.inject.apt_mirror = url.to_owned();
                            }
                        }
                    });
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.apt_mirror)
                            .hint_text("http://archive.ubuntu.com/ubuntu")
                            .desired_width(f32::INFINITY)
                            .min_size(Vec2::new(0.0, 34.0)),
                    );
                    ui.add_space(8.0);
                    lbl(ui, "Extra APT Repos  (one per line — click to add)");
                    ui.horizontal_wrapped(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 6.0;
                        for (label, repo) in [
                            ("universe",     "deb http://archive.ubuntu.com/ubuntu noble universe"),
                            ("multiverse",   "deb http://archive.ubuntu.com/ubuntu noble multiverse"),
                            ("Docker CE",    "deb [arch=amd64 signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu noble stable"),
                            ("GitHub CLI",   "deb [arch=amd64 signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main"),
                            ("HashiCorp",    "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com noble main"),
                            ("NodeSource 20","deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_20.x nodistro main"),
                            ("MongoDB 7",    "deb [signed-by=/usr/share/keyrings/mongodb-server-7.0.gpg] https://repo.mongodb.org/apt/ubuntu noble/mongodb-org/7.0 multiverse"),
                            ("Grafana",      "deb [signed-by=/usr/share/keyrings/grafana.key] https://apt.grafana.com stable main"),
                        ] {
                            if ui.add_enabled(
                                !running,
                                egui::Button::new(label)
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(0.0, 28.0)),
                            ).on_hover_text(repo).clicked() && !self.inject.apt_repos.contains(repo) {
                                if !self.inject.apt_repos.is_empty()
                                    && !self.inject.apt_repos.ends_with('\n')
                                {
                                    self.inject.apt_repos.push('\n');
                                }
                                self.inject.apt_repos.push_str(repo);
                            }
                        }
                    });
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.apt_repos)
                            .hint_text("ppa:ondrej/php\ndeb http://archive.ubuntu.com/ubuntu noble universe")
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                });
                ui.end_row();
            });

        ui.add_space(12.0);

        // Row 2: DNF (Fedora) + Pacman (Arch)
        egui::Grid::new("adv_dnf_pacman_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "DNF Mirror  (Fedora/RHEL — click to preset)");
                    ui.horizontal_wrapped(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 6.0;
                        for (label, url) in [
                            ("official",  "https://download.fedoraproject.org/pub/fedora/linux"),
                            ("MIT",       "https://mirrors.mit.edu/fedora/linux"),
                            ("OSUOSL",    "https://ftp.osuosl.org/pub/fedora/linux"),
                            ("kernel.org","https://mirrors.kernel.org/fedora"),
                        ] {
                            if ui.add_enabled(
                                !running,
                                egui::Button::new(label)
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(0.0, 28.0)),
                            ).on_hover_text(url).clicked() {
                                self.inject.dnf_mirror = url.to_owned();
                            }
                        }
                    });
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.dnf_mirror)
                            .hint_text("https://download.fedoraproject.org/pub/fedora/linux")
                            .desired_width(f32::INFINITY)
                            .min_size(Vec2::new(0.0, 34.0)),
                    );
                    ui.add_space(8.0);
                    lbl(ui, "Extra DNF Repos  (URL or stanza — click to add)");
                    ui.horizontal_wrapped(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 6.0;
                        for (label, repo) in [
                            ("EPEL",              "https://dl.fedoraproject.org/pub/epel/epel-release-latest-${releasever}.noarch.rpm"),
                            ("RPMFusion Free",    "https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-${releasever}.noarch.rpm"),
                            ("RPMFusion NonFree", "https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-${releasever}.noarch.rpm"),
                            ("Docker CE",         "https://download.docker.com/linux/fedora/docker-ce.repo"),
                            ("HashiCorp",         "https://rpm.releases.hashicorp.com/fedora/hashicorp.repo"),
                            ("GitHub CLI",        "https://cli.github.com/packages/rpm/gh-cli.repo"),
                            ("VS Code",           "https://packages.microsoft.com/yumrepos/vscode"),
                            ("Google Chrome",     "https://dl.google.com/linux/chrome/rpm/stable/x86_64"),
                            ("NodeSource 20",     "https://rpm.nodesource.com/pub_20.x/nodistro/nodejs/${arch}/nodesource-release-nodistro-1.noarch.rpm"),
                            ("Grafana",           "https://rpm.grafana.com/oss/release"),
                        ] {
                            if ui.add_enabled(
                                !running,
                                egui::Button::new(label)
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(0.0, 28.0)),
                            ).on_hover_text(repo).clicked() && !self.inject.dnf_repos.contains(repo) {
                                if !self.inject.dnf_repos.is_empty()
                                    && !self.inject.dnf_repos.ends_with('\n')
                                {
                                    self.inject.dnf_repos.push('\n');
                                }
                                self.inject.dnf_repos.push_str(repo);
                            }
                        }
                    });
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.dnf_repos)
                            .hint_text("https://dl.fedoraproject.org/pub/epel/epel-release-latest-${releasever}.noarch.rpm")
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Pacman Mirror  (Arch Linux — click to preset)");
                    ui.horizontal_wrapped(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 6.0;
                        for (label, url) in [
                            ("Cloudflare", "https://cloudflaremirrors.com/archlinux"),
                            ("MIT",        "https://mirrors.mit.edu/archlinux"),
                            ("OSUOSL",     "https://ftp.osuosl.org/pub/archlinux"),
                            ("kernel.org", "https://mirrors.edge.kernel.org/archlinux"),
                            ("Rackspace",  "https://mirror.rackspace.com/archlinux"),
                        ] {
                            if ui.add_enabled(
                                !running,
                                egui::Button::new(label)
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(0.0, 28.0)),
                            ).on_hover_text(url).clicked() {
                                self.inject.pacman_mirror = url.to_owned();
                            }
                        }
                    });
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.pacman_mirror)
                            .hint_text("https://cloudflaremirrors.com/archlinux")
                            .desired_width(f32::INFINITY)
                            .min_size(Vec2::new(0.0, 34.0)),
                    );
                    ui.add_space(8.0);
                    lbl(ui, "Extra Pacman Repos  (Server = lines — click to add)");
                    ui.horizontal_wrapped(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 6.0;
                        for (label, repo) in [
                            ("Cloudflare",  "Server = https://cloudflaremirrors.com/archlinux/$repo/os/$arch"),
                            ("kernel.org",  "Server = https://mirrors.edge.kernel.org/archlinux/$repo/os/$arch"),
                            ("MIT",         "Server = https://mirrors.mit.edu/archlinux/$repo/os/$arch"),
                            ("OSUOSL",      "Server = https://ftp.osuosl.org/pub/archlinux/$repo/os/$arch"),
                            ("Rackspace",   "Server = https://mirror.rackspace.com/archlinux/$repo/os/$arch"),
                            ("Chaotic-AUR", "Server = https://cdn-mirror.chaotic.cx/$repo/$arch"),
                            ("ArchLinuxCN", "Server = https://repo.archlinuxcn.org/$arch"),
                            ("BlackArch",   "Server = https://blackarch.org/blackarch/$repo/os/$arch"),
                        ] {
                            if ui.add_enabled(
                                !running,
                                egui::Button::new(label)
                                    .fill(SURFACE)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .min_size(Vec2::new(0.0, 28.0)),
                            ).on_hover_text(repo).clicked() && !self.inject.pacman_repos.contains(repo) {
                                if !self.inject.pacman_repos.is_empty()
                                    && !self.inject.pacman_repos.ends_with('\n')
                                {
                                    self.inject.pacman_repos.push('\n');
                                }
                                self.inject.pacman_repos.push_str(repo);
                            }
                        }
                    });
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.pacman_repos)
                            .hint_text("Server = https://cloudflaremirrors.com/archlinux/$repo/os/$arch")
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                });
                ui.end_row();
            });

        rule(ui);

        // ── Commands ──────────────────────────────────────────────────────
        section(ui, "Run Commands");
        egui::Grid::new("adv_cmd_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "Early Commands  (run before packages)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.run_commands)
                            .hint_text("apt-get update -qq")
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Late Commands  (run at end of install)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.late_commands)
                            .hint_text("systemctl enable myservice")
                            .desired_width(f32::INFINITY)
                            .desired_rows(4),
                    );
                });
                ui.end_row();
            });

        rule(ui);

        // ── Services & Containers ─────────────────────────────────────────
        section(ui, "Services & Containers");
        egui::Grid::new("adv_svc_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "Enable Services  (one per line)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.enable_services)
                            .hint_text("docker\nssh")
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Disable Services  (one per line)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.disable_services)
                            .hint_text("snapd")
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                });
                ui.end_row();
                ui.vertical(|ui| {
                    lbl(ui, "User Groups  (one per line)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.user_groups)
                            .hint_text("docker\nsudo")
                            .desired_width(f32::INFINITY)
                            .desired_rows(3),
                    );
                });
                ui.vertical(|ui| {
                    ui.add_space(16.0);
                    ui.add_enabled(
                        !running,
                        egui::Checkbox::new(&mut self.inject.docker, "Install Docker"),
                    );
                    ui.add_space(4.0);
                    ui.add_enabled(
                        !running,
                        egui::Checkbox::new(&mut self.inject.podman, "Install Podman"),
                    );
                });
                ui.end_row();
            });

        rule(ui);

        // ── Firewall ──────────────────────────────────────────────────────
        section(ui, "Firewall  (UFW)");
        ui.horizontal(|ui| {
            ui.add_enabled(
                !running,
                egui::Checkbox::new(&mut self.inject.firewall_enabled, "Enable UFW"),
            );
            if self.inject.firewall_enabled {
                ui.add_space(16.0);
                lbl(ui, "Default policy:");
                ui.add_space(4.0);
                ui.add_enabled(
                    !running,
                    egui::TextEdit::singleline(&mut self.inject.firewall_policy)
                        .hint_text("deny")
                        .desired_width(80.0),
                );
            }
        });
        if self.inject.firewall_enabled {
            ui.add_space(6.0);
            egui::Grid::new("adv_fw_grid")
                .num_columns(2)
                .spacing([20.0, 16.0])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        lbl(ui, "Allow Ports  (one per line, e.g. 22/tcp)");
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::multiline(&mut self.inject.allow_ports)
                                .hint_text("22/tcp\n443")
                                .desired_width(f32::INFINITY)
                                .desired_rows(4),
                        );
                    });
                    ui.vertical(|ui| {
                        lbl(ui, "Deny Ports  (one per line)");
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::multiline(&mut self.inject.deny_ports)
                                .hint_text("23")
                                .desired_width(f32::INFINITY)
                                .desired_rows(4),
                        );
                    });
                    ui.end_row();
                });
        }

        rule(ui);

        // ── Boot & Storage ────────────────────────────────────────────────
        section(ui, "Boot & Storage");
        egui::Grid::new("adv_boot_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "GRUB Timeout  (seconds)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.grub_timeout)
                            .hint_text("5")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "GRUB Default Entry");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.grub_default)
                            .hint_text("Ubuntu")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
                ui.vertical(|ui| {
                    lbl(ui, "Extra Kernel Cmdline Args");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.grub_cmdline)
                            .hint_text("quiet splash")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Swap Size  (MB, blank = none)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.swap_size_mb)
                            .hint_text("2048")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
            });
        lbl(ui, "Sysctl  (key=value, one per line)");
        ui.add_enabled(
            !running,
            egui::TextEdit::multiline(&mut self.inject.sysctl_pairs)
                .hint_text("net.ipv4.ip_forward=1\nvm.swappiness=10")
                .desired_width(full_w)
                .desired_rows(4),
        );

        rule(ui);

        // ── Output Options ────────────────────────────────────────────────
        section(ui, "Output Options");
        egui::Grid::new("adv_out_grid")
            .num_columns(2)
            .min_col_width(col_w)
            .spacing([20.0, 16.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    lbl(ui, "Volume Label  (blank = keep original)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.output_label)
                            .hint_text("MY-UBUNTU")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.vertical(|ui| {
                    lbl(ui, "Expected SHA-256  (blank = skip check)");
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.inject.expected_sha256)
                            .hint_text("64-char hex")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.add_enabled(
            !running,
            egui::Checkbox::new(
                &mut self.inject.no_user_interaction,
                "No user interaction  (fully unattended install)",
            ),
        );
        ui.add_space(6.0);
    }

    fn show_verify(&mut self, ui: &mut Ui) {
        let running = self.job_running;
        let mut do_verify = false;
        let mut do_9660 = false;

        egui::ScrollArea::vertical()
            .id_salt("verify_scroll")
            .show(ui, |ui| {
                ui.add_space(12.0);

                // ── SHA-256 Checksum ────────────────────────────────────
                ui.label(
                    RichText::new("SHA-256 Checksum Verification")
                        .size(17.0)
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Verifies an ISO against its official SHA256SUMS file. \
                         Auto-detected for Ubuntu. For injected or renamed ISOs, \
                         the computed hash is displayed for your records.",
                    )
                    .size(14.0)
                    .color(MUTED),
                );
                ui.add_space(10.0);

                lbl(ui, "ISO Path  (local path or URL)");
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.verify.source)
                            .hint_text("/path/to/ubuntu.iso")
                            .desired_width(ui.available_width() - 96.0)
                            .min_size(Vec2::new(0.0, 38.0)),
                    );
                    if browse_btn(ui, !running) {
                        worker::pick_iso(PickTarget::VerifySource, self.tx.clone());
                    }
                });

                ui.add_space(6.0);
                lbl(ui, "SHA256SUMS URL  (optional — auto-detected for Ubuntu)");
                ui.add_enabled(
                    !running,
                    egui::TextEdit::singleline(&mut self.verify.sums_url)
                        .hint_text("https://releases.ubuntu.com/24.04/SHA256SUMS")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(12.0);
                let can_verify = !self.verify.source.trim().is_empty() && !running;
                let verify_lbl = if running && self.job_phase.to_lowercase().contains("verify") {
                    "⏳  Verifying…"
                } else {
                    "Verify Checksum"
                };
                if action_btn(ui, verify_lbl, can_verify) {
                    do_verify = true;
                }

                // ── Verify result ────────────────────────────────────────
                if let Some(r) = self.verify_result.clone() {
                    ui.add_space(12.0);
                    let (fill, border, icon) = if r.matched {
                        (
                            Color32::from_rgb(13, 28, 18),
                            Color32::from_rgb(40, 100, 55),
                            "✅",
                        )
                    } else {
                        (
                            Color32::from_rgb(30, 15, 15),
                            Color32::from_rgb(100, 40, 40),
                            "⚠️",
                        )
                    };
                    result_box(ui, fill, border, |ui| {
                        let col = if r.matched { GREEN } else { AMBER };
                        ui.label(
                            RichText::new(format!(
                                "{}  {}",
                                icon,
                                if r.matched {
                                    "Checksum Matched"
                                } else {
                                    "Checksum Not Matched"
                                }
                            ))
                            .size(14.0)
                            .strong()
                            .color(col),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("File:    ")
                                    .size(14.0)
                                    .monospace()
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(&r.filename)
                                    .size(14.0)
                                    .monospace()
                                    .color(TEXT),
                            );
                        });
                        // Expected
                        let exp_display = if r.expected.len() == 64
                            && r.expected.chars().all(|c| c.is_ascii_hexdigit())
                        {
                            format!("{}…", &r.expected[..32])
                        } else {
                            r.expected.clone()
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Expected:")
                                    .size(14.0)
                                    .monospace()
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(exp_display)
                                    .size(14.0)
                                    .monospace()
                                    .color(MUTED),
                            );
                        });
                        // Actual
                        let act_col = if r.matched { GREEN } else { AMBER };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Actual:  ")
                                    .size(14.0)
                                    .monospace()
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(format!("{}…", &r.actual[..32.min(r.actual.len())]))
                                    .size(14.0)
                                    .monospace()
                                    .color(act_col),
                            );
                            if ui
                                .small_button("📋")
                                .on_hover_text("Copy full SHA-256")
                                .clicked()
                            {
                                ui.ctx().copy_text(r.actual.clone());
                            }
                        });
                    });
                }

                rule(ui);

                // ── ISO-9660 Validation ──────────────────────────────────
                ui.label(
                    RichText::new("ISO-9660 Structure Validation")
                        .size(17.0)
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Checks that the ISO has a valid ISO-9660 filesystem header. \
                         Uses the same source path as checksum verification above.",
                    )
                    .size(14.0)
                    .color(MUTED),
                );
                ui.add_space(10.0);
                let can_9660 = !self.verify.source.trim().is_empty() && !running;
                let iso_lbl = if running && self.job_phase.to_lowercase().contains("9660") {
                    "⏳  Validating…"
                } else {
                    "Validate ISO-9660"
                };
                if small_btn(ui, iso_lbl, can_9660) {
                    do_9660 = true;
                }

                if let Some(r) = self.iso9660_result.clone() {
                    ui.add_space(8.0);
                    let (fill, border, msg, col) = if r.compliant {
                        (
                            Color32::from_rgb(13, 28, 18),
                            Color32::from_rgb(40, 100, 55),
                            "✅  ISO-9660 Compliant",
                            GREEN,
                        )
                    } else {
                        (
                            Color32::from_rgb(30, 15, 15),
                            Color32::from_rgb(100, 40, 40),
                            "❌  Not Compliant",
                            RED,
                        )
                    };
                    result_box(ui, fill, border, |ui| {
                        ui.label(RichText::new(msg).size(13.0).strong().color(col));
                        if let Some(vid) = &r.volume_id {
                            ui.label(
                                RichText::new(format!("Volume ID: {vid}"))
                                    .size(14.0)
                                    .color(MUTED),
                            );
                        }
                        if let Some(err) = &r.error {
                            ui.label(RichText::new(err).size(13.0).color(RED));
                        }
                    });
                }

                ui.add_space(16.0);
            });

        if do_verify {
            self.spawn_verify();
        }
        if do_9660 {
            self.spawn_iso9660();
        }
    }

    fn show_diff(&mut self, ui: &mut Ui) {
        let running = self.job_running;
        let mut do_diff = false;

        egui::ScrollArea::vertical()
            .id_salt("diff_scroll")
            .show(ui, |ui| {
                ui.add_space(12.0);

                ui.label(
                    RichText::new("Compare Two ISO Images")
                        .size(17.0)
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Select the original (base) and modified (target) ISOs \
                         to see what files were added, removed, or changed.",
                    )
                    .size(14.0)
                    .color(MUTED),
                );
                ui.add_space(12.0);

                let full_w = ui.available_width();
                let col_w = (full_w - 20.0) / 2.0;

                egui::Grid::new("diff_paths")
                    .num_columns(2)
                    .min_col_width(col_w)
                    .spacing([20.0, 12.0])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            lbl(ui, "Base ISO  (original)");
                            ui.horizontal(|ui| {
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.diff.base)
                                        .hint_text("/path/to/original.iso")
                                        .desired_width(f32::INFINITY)
                                        .min_size(Vec2::new(0.0, 38.0)),
                                );
                                if ui
                                    .add_enabled(
                                        !running,
                                        egui::Button::new("📂")
                                            .fill(SURFACE)
                                            .stroke(Stroke::new(1.0, BORDER))
                                            .min_size(Vec2::new(40.0, 38.0)),
                                    )
                                    .on_hover_text("Browse for base ISO")
                                    .clicked()
                                {
                                    worker::pick_iso(PickTarget::DiffBase, self.tx.clone());
                                }
                            });
                        });
                        ui.vertical(|ui| {
                            lbl(ui, "Target ISO  (modified)");
                            ui.horizontal(|ui| {
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.diff.target)
                                        .hint_text("/path/to/modified.iso")
                                        .desired_width(f32::INFINITY)
                                        .min_size(Vec2::new(0.0, 38.0)),
                                );
                                if ui
                                    .add_enabled(
                                        !running,
                                        egui::Button::new("📂")
                                            .fill(SURFACE)
                                            .stroke(Stroke::new(1.0, BORDER))
                                            .min_size(Vec2::new(40.0, 38.0)),
                                    )
                                    .on_hover_text("Browse for target ISO")
                                    .clicked()
                                {
                                    worker::pick_iso(PickTarget::DiffTarget, self.tx.clone());
                                }
                            });
                        });
                        ui.end_row();
                    });

                ui.add_space(12.0);
                let can = !self.diff.base.trim().is_empty()
                    && !self.diff.target.trim().is_empty()
                    && !running;
                let diff_lbl = if running {
                    "⏳  Comparing…"
                } else {
                    "Compare ISOs"
                };
                if action_btn(ui, diff_lbl, can) {
                    do_diff = true;
                }

                // ── Results ──────────────────────────────────────────────
                if let Some(r) = self.diff_result.clone() {
                    let added = r.added.len();
                    let removed = r.removed.len();
                    let modified = r.modified.len();
                    let unchanged = r.unchanged;

                    ui.add_space(16.0);

                    // Summary row
                    ui.horizontal(|ui| {
                        for (n, label, col) in [
                            (added, "Added", GREEN),
                            (removed, "Removed", RED),
                            (modified, "Modified", AMBER),
                            (unchanged, "Unchanged", MUTED),
                        ] {
                            Frame::new()
                                .fill(SURFACE)
                                .stroke(Stroke::new(1.0, BORDER))
                                .inner_margin(12.0f32)
                                .corner_radius(6.0f32)
                                .show(ui, |ui| {
                                    ui.set_min_width(90.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            RichText::new(n.to_string())
                                                .size(26.0)
                                                .strong()
                                                .color(col),
                                        );
                                        ui.label(RichText::new(label).size(13.0).color(MUTED));
                                    });
                                });
                            ui.add_space(8.0);
                        }
                    });

                    ui.add_space(10.0);

                    // Filter + search
                    ui.horizontal(|ui| {
                        for (filter, label) in [
                            (DiffFilter::All, "All"),
                            (DiffFilter::Added, "Added"),
                            (DiffFilter::Removed, "Removed"),
                            (DiffFilter::Modified, "Modified"),
                        ] {
                            let active = self.diff_filter == filter;
                            let fill = if active { ACCENT } else { SURFACE };
                            let col = if active { Color32::WHITE } else { MUTED };
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(label).size(14.0).color(col))
                                        .fill(fill)
                                        .stroke(Stroke::new(
                                            1.0,
                                            if active { ACCENT } else { BORDER },
                                        ))
                                        .min_size(Vec2::new(90.0, 38.0)),
                                )
                                .clicked()
                            {
                                self.diff_filter = filter;
                            }
                            ui.add_space(6.0);
                        }
                        ui.add_space(16.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.diff_search)
                                .hint_text("Filter paths…")
                                .desired_width(280.0)
                                .min_size(Vec2::new(0.0, 38.0)),
                        );
                    });

                    ui.add_space(6.0);

                    egui::ScrollArea::vertical()
                        .id_salt("diff_results_scroll")
                        .max_height(480.0)
                        .show(ui, |ui| {
                            let search = self.diff_search.to_lowercase();

                            // Added
                            if matches!(self.diff_filter, DiffFilter::All | DiffFilter::Added) {
                                for p in &r.added {
                                    let s = p.to_lowercase();
                                    if !search.is_empty() && !s.contains(&search) {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("+").size(14.0).monospace().color(GREEN),
                                        );
                                        ui.label(
                                            RichText::new(p).size(14.0).monospace().color(TEXT),
                                        );
                                    });
                                }
                            }

                            // Removed
                            if matches!(self.diff_filter, DiffFilter::All | DiffFilter::Removed) {
                                for p in &r.removed {
                                    let s = p.to_lowercase();
                                    if !search.is_empty() && !s.contains(&search) {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("-").size(14.0).monospace().color(RED),
                                        );
                                        ui.label(
                                            RichText::new(p).size(14.0).monospace().color(TEXT),
                                        );
                                    });
                                }
                            }

                            // Modified
                            if matches!(self.diff_filter, DiffFilter::All | DiffFilter::Modified) {
                                for entry in &r.modified {
                                    let s = entry.path.to_lowercase();
                                    if !search.is_empty() && !s.contains(&search) {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("~").size(14.0).monospace().color(AMBER),
                                        );
                                        ui.label(
                                            RichText::new(&entry.path)
                                                .size(14.0)
                                                .monospace()
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "  {} → {}",
                                                fmt_bytes(entry.base_size),
                                                fmt_bytes(entry.target_size)
                                            ))
                                            .size(13.0)
                                            .color(MUTED),
                                        );
                                    });
                                }
                            }
                        });
                }

                ui.add_space(16.0);
            });

        if do_diff {
            self.spawn_diff();
        }
    }

    fn show_build(&mut self, ui: &mut Ui) {
        let running = self.job_running;
        let mut do_build = false;
        let mut do_inspect = false;

        egui::ScrollArea::vertical()
            .id_salt("build_scroll")
            .show(ui, |ui| {
                ui.add_space(12.0);

                ui.label(
                    RichText::new("Fetch & Build ISO")
                        .size(17.0)
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Download, verify, and repack an ISO with optional overlay files \
                         and configuration.",
                    )
                    .size(14.0)
                    .color(MUTED),
                );
                ui.add_space(12.0);

                let full_w = ui.available_width();
                let col_w = (full_w - 20.0) / 2.0;

                lbl(ui, "Source ISO  (local path or URL)");
                ui.horizontal(|ui| {
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::singleline(&mut self.build.source)
                            .hint_text("/path/to/ubuntu.iso  or  https://releases.ubuntu.com/…")
                            .desired_width(ui.available_width() - 96.0)
                            .min_size(Vec2::new(0.0, 38.0)),
                    );
                    if browse_btn(ui, !running) {
                        worker::pick_iso(PickTarget::BuildSource, self.tx.clone());
                    }
                });

                ui.add_space(8.0);

                egui::Grid::new("build_grid")
                    .num_columns(2)
                    .min_col_width(col_w)
                    .spacing([20.0, 16.0])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            lbl(ui, "Output Directory");
                            ui.horizontal(|ui| {
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.build.output_dir)
                                        .desired_width(f32::INFINITY)
                                        .min_size(Vec2::new(0.0, 38.0)),
                                );
                                if ui
                                    .add_enabled(
                                        !running,
                                        egui::Button::new("📂")
                                            .fill(SURFACE)
                                            .stroke(Stroke::new(1.0, BORDER))
                                            .min_size(Vec2::new(40.0, 38.0)),
                                    )
                                    .clicked()
                                {
                                    worker::pick_folder(
                                        PickTarget::BuildOutputDir,
                                        self.tx.clone(),
                                    );
                                }
                            });
                        });
                        ui.vertical(|ui| {
                            lbl(ui, "Build Name");
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.build.build_name)
                                    .hint_text("forgeiso-local")
                                    .desired_width(f32::INFINITY),
                            );
                        });
                        ui.end_row();
                        ui.vertical(|ui| {
                            lbl(ui, "Overlay Directory  (optional)");
                            ui.horizontal(|ui| {
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.build.overlay_dir)
                                        .hint_text("/path/to/overlay/")
                                        .desired_width(f32::INFINITY),
                                );
                                if ui
                                    .add_enabled(
                                        !running,
                                        egui::Button::new("📂")
                                            .fill(SURFACE)
                                            .stroke(Stroke::new(1.0, BORDER))
                                            .min_size(Vec2::new(40.0, 38.0)),
                                    )
                                    .clicked()
                                {
                                    worker::pick_folder(PickTarget::BuildOverlay, self.tx.clone());
                                }
                            });
                        });
                        ui.vertical(|ui| {
                            lbl(ui, "Volume Label  (optional)");
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.build.output_label)
                                    .hint_text("MY-UBUNTU")
                                    .desired_width(f32::INFINITY),
                            );
                        });
                        ui.end_row();
                        ui.vertical(|ui| {
                            lbl(ui, "Profile");
                            egui::ComboBox::from_id_salt("profile_combo")
                                .selected_text(if self.build.profile == "desktop" {
                                    "Desktop"
                                } else {
                                    "Minimal"
                                })
                                .width(col_w)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.build.profile,
                                        "minimal".into(),
                                        "Minimal",
                                    );
                                    ui.selectable_value(
                                        &mut self.build.profile,
                                        "desktop".into(),
                                        "Desktop",
                                    );
                                });
                        });
                        ui.vertical(|ui| {
                            lbl(ui, "Expected SHA-256  (optional)");
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.build.expected_sha256)
                                    .hint_text("64-char hex")
                                    .desired_width(f32::INFINITY),
                            );
                        });
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let can_build = !self.build.source.trim().is_empty()
                        && !self.build.output_dir.trim().is_empty()
                        && !running;
                    let build_lbl = if running {
                        "⏳  Building…"
                    } else {
                        "Build ISO"
                    };
                    let btn = egui::Button::new(
                        RichText::new(build_lbl)
                            .size(14.0)
                            .strong()
                            .color(if can_build { Color32::WHITE } else { MUTED }),
                    )
                    .fill(if can_build { ACCENT } else { SURFACE })
                    .stroke(Stroke::new(1.0, if can_build { ACCENT } else { BORDER }))
                    .min_size(Vec2::new(180.0, 48.0));
                    if ui.add_enabled(can_build, btn).clicked() {
                        do_build = true;
                    }
                    ui.add_space(12.0);
                    let can_inspect = !self.build.source.trim().is_empty() && !running;
                    if small_btn(ui, "Inspect ISO", can_inspect) {
                        do_inspect = true;
                    }
                });

                // ── Inspect result ───────────────────────────────────────
                if let Some(m) = self.inspect_result.clone() {
                    ui.add_space(12.0);
                    result_box(ui, SURFACE, BORDER, |ui| {
                        ui.label(
                            RichText::new("ISO Information")
                                .size(13.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.add_space(4.0);
                        for (k, v) in [
                            (
                                "Distro",
                                m.distro.as_ref().map(distro_label).unwrap_or_default(),
                            ),
                            ("Release", m.release.clone().unwrap_or_default()),
                            ("Arch", m.architecture.clone().unwrap_or_default()),
                            ("Size", fmt_bytes(m.size_bytes)),
                            (
                                "Volume ID",
                                m.volume_id.clone().unwrap_or_else(|| "—".into()),
                            ),
                            (
                                "SHA-256",
                                format!("{}…", &m.sha256[..32.min(m.sha256.len())]),
                            ),
                            (
                                "Boot",
                                format!(
                                    "{}{}",
                                    if m.boot.bios { "BIOS " } else { "" },
                                    if m.boot.uefi { "UEFI" } else { "" }
                                ),
                            ),
                        ] {
                            if v.is_empty() {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{k}:")).size(14.0).color(MUTED));
                                ui.label(RichText::new(&v).size(14.0).monospace().color(TEXT));
                            });
                        }
                    });
                }

                // ── Build result ─────────────────────────────────────────
                if let Some(r) = self.build_result.clone() {
                    ui.add_space(12.0);
                    card_green(ui, |ui| {
                        ui.label(
                            RichText::new("✓  Build Complete")
                                .size(14.0)
                                .strong()
                                .color(GREEN),
                        );
                        ui.add_space(6.0);
                        for a in &r.artifacts {
                            let path_str = a.to_string_lossy();
                            let avail = ui.available_width();
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut path_str.as_ref().to_string())
                                        .desired_width(avail - 130.0)
                                        .interactive(false)
                                        .font(egui::FontId::monospace(12.0)),
                                );
                                if ui
                                    .add(
                                        egui::Button::new("📋 Copy")
                                            .fill(SURFACE)
                                            .stroke(Stroke::new(1.0, BORDER)),
                                    )
                                    .clicked()
                                {
                                    ui.ctx().copy_text(path_str.into_owned());
                                }
                                if ui
                                    .add(
                                        egui::Button::new("📂 Open")
                                            .fill(SURFACE)
                                            .stroke(Stroke::new(1.0, BORDER)),
                                    )
                                    .clicked()
                                {
                                    if let Some(dir) = a.parent() {
                                        if std::process::Command::new("xdg-open")
                                            .arg(dir)
                                            .spawn()
                                            .is_err()
                                        {
                                            self.set_status(StatusMsg::err(
                                                "xdg-open failed — open the folder manually",
                                            ));
                                        }
                                    }
                                }
                            });
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if small_btn(ui, "Scan", !running) {
                                self.spawn_scan();
                            }
                            ui.add_space(8.0);
                            if small_btn(ui, "Boot Test", !running) {
                                self.spawn_test_iso();
                            }
                            ui.add_space(8.0);
                            if small_btn(ui, "HTML Report", !running) {
                                self.spawn_report("html");
                            }
                            ui.add_space(8.0);
                            if small_btn(ui, "JSON Report", !running) {
                                self.spawn_report("json");
                            }
                        });
                    });
                }

                ui.add_space(16.0);
            });

        if do_build {
            self.spawn_build();
        }
        if do_inspect {
            self.spawn_inspect();
        }
    }

    fn show_doctor(&mut self, ui: &mut Ui) {
        let running = self.job_running;

        egui::ScrollArea::vertical()
            .id_salt("doctor_scroll")
            .show(ui, |ui| {
                ui.add_space(12.0);

                ui.label(
                    RichText::new("System Dependencies")
                        .size(17.0)
                        .strong()
                        .color(TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Checks that all required tools (xorriso, grub, squashfs-tools, etc.) \
                         are installed and accessible.",
                    )
                    .size(14.0)
                    .color(MUTED),
                );
                ui.add_space(10.0);

                let lbl = if running {
                    "⏳  Checking…"
                } else {
                    "Run Dependency Check"
                };
                if small_btn(ui, lbl, !running) {
                    self.spawn_doctor();
                }

                if let Some(r) = self.doctor_result.clone() {
                    ui.add_space(12.0);
                    let all_ok = r.tooling.values().all(|&ok| ok);
                    let (fill, border) = if all_ok {
                        (
                            Color32::from_rgb(13, 28, 18),
                            Color32::from_rgb(40, 100, 55),
                        )
                    } else {
                        (
                            Color32::from_rgb(28, 18, 10),
                            Color32::from_rgb(100, 60, 20),
                        )
                    };

                    result_box(ui, fill, border, |ui| {
                        ui.label(
                            RichText::new(if all_ok {
                                "✅  All dependencies satisfied"
                            } else {
                                "⚠️  Some dependencies missing"
                            })
                            .size(13.0)
                            .strong()
                            .color(if all_ok { GREEN } else { AMBER }),
                        );
                        ui.add_space(8.0);
                        egui::Grid::new("doctor_grid")
                            .num_columns(2)
                            .spacing([24.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Tool").size(14.0).strong().color(MUTED));
                                ui.label(RichText::new("Status").size(14.0).strong().color(MUTED));
                                ui.end_row();
                                for (name, &ok) in &r.tooling {
                                    ui.label(
                                        RichText::new(name).size(15.0).monospace().color(TEXT),
                                    );
                                    let (status_text, status_col) = if ok {
                                        ("✓ OK", GREEN)
                                    } else {
                                        ("✗ Missing", RED)
                                    };
                                    ui.label(
                                        RichText::new(status_text).size(15.0).color(status_col),
                                    );
                                    ui.end_row();
                                }
                            });
                    });

                    if !all_ok {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Install missing tools with:")
                                .size(14.0)
                                .color(MUTED),
                        );
                        ui.add_space(6.0);
                        Frame::new()
                            .fill(Color32::from_rgb(10, 14, 20))
                            .stroke(Stroke::new(1.0, BORDER))
                            .inner_margin(12.0f32)
                            .corner_radius(6.0f32)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(
                                        "sudo dnf install xorriso grub2-tools squashfs-tools",
                                    )
                                    .size(15.0)
                                    .monospace()
                                    .color(TEXT),
                                );
                            });
                    }
                }

                ui.add_space(16.0);
            });
    }
}

// ── eframe::App impl ─────────────────────────────────────────────────────────

impl eframe::App for ForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages(ctx);

        // Request repaint while a job is running so progress shows smoothly
        if self.job_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.render_header(ctx);
        self.render_tabs(ctx);
        self.render_log(ctx);

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(28, 20)),
            )
            .show(ctx, |ui| match self.active_tab {
                Tab::Inject => self.show_inject(ui),
                Tab::Verify => self.show_verify(ui),
                Tab::Diff => self.show_diff(ui),
                Tab::Build => self.show_build(ui),
                Tab::Doctor => self.show_doctor(ui),
            });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = PersistedState {
            inject: self.inject.clone(),
            verify: self.verify.clone(),
            diff: self.diff.clone(),
            build: self.build.clone(),
        };
        eframe::set_value(storage, STORAGE_KEY, &state);
    }
}

// ── Build InjectConfig from form state ────────────────────────────────────────

fn build_inject_config(inject: &InjectState) -> InjectConfig {
    let distro = match inject.distro.as_str() {
        "fedora" => Some(Distro::Fedora),
        "arch" => Some(Distro::Arch),
        "mint" => Some(Distro::Mint),
        _ => None,
    };

    InjectConfig {
        source: IsoSource::from_raw(&inject.source),
        out_name: inject.out_name.clone(),
        output_label: opt(&inject.output_label),
        autoinstall_yaml: None,
        hostname: opt(&inject.hostname),
        username: opt(&inject.username),
        password: opt(&inject.password),
        realname: opt(&inject.realname),
        ssh: SshConfig {
            authorized_keys: lines(&inject.ssh_keys),
            allow_password_auth: Some(inject.ssh_password_auth),
            install_server: Some(inject.ssh_install_server),
        },
        network: NetworkConfig {
            dns_servers: lines(&inject.dns_servers),
            ntp_servers: lines(&inject.ntp_servers),
        },
        static_ip: opt(&inject.static_ip),
        gateway: opt(&inject.gateway),
        proxy: ProxyConfig {
            http_proxy: opt(&inject.http_proxy),
            https_proxy: opt(&inject.https_proxy),
            no_proxy: inject
                .no_proxy
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        },
        timezone: opt(&inject.timezone),
        locale: opt(&inject.locale),
        keyboard_layout: opt(&inject.keyboard_layout),
        storage_layout: opt(&inject.storage_layout),
        apt_mirror: opt(&inject.apt_mirror),
        extra_packages: lines(&inject.packages),
        wallpaper: opt(&inject.wallpaper_path).map(PathBuf::from),
        extra_late_commands: lines(&inject.late_commands),
        no_user_interaction: inject.no_user_interaction,
        user: UserConfig {
            groups: lines(&inject.user_groups),
            ..Default::default()
        },
        firewall: FirewallConfig {
            enabled: inject.firewall_enabled,
            default_policy: opt(&inject.firewall_policy),
            allow_ports: lines(&inject.allow_ports),
            deny_ports: lines(&inject.deny_ports),
        },
        enable_services: lines(&inject.enable_services),
        disable_services: lines(&inject.disable_services),
        sysctl: lines(&inject.sysctl_pairs)
            .iter()
            .filter_map(|s| {
                let mut parts = s.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some(k), Some(v)) => Some((k.trim().to_string(), v.trim().to_string())),
                    _ => None,
                }
            })
            .collect(),
        swap: inject
            .swap_size_mb
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|&n| n > 0)
            .map(|size_mb| SwapConfig {
                size_mb,
                filename: None,
                swappiness: None,
            }),
        apt_repos: lines(&inject.apt_repos),
        dnf_repos: lines(&inject.dnf_repos),
        dnf_mirror: opt(&inject.dnf_mirror),
        pacman_repos: lines(&inject.pacman_repos),
        pacman_mirror: opt(&inject.pacman_mirror),
        containers: ContainerConfig {
            docker: inject.docker,
            podman: inject.podman,
            docker_users: Vec::new(),
        },
        grub: GrubConfig {
            timeout: inject.grub_timeout.trim().parse::<u32>().ok(),
            cmdline_extra: inject
                .grub_cmdline
                .split_whitespace()
                .map(String::from)
                .collect(),
            default_entry: opt(&inject.grub_default),
        },
        encrypt: false,
        encrypt_passphrase: None,
        mounts: Vec::new(),
        run_commands: lines(&inject.run_commands),
        distro,
        expected_sha256: opt(&inject.expected_sha256),
    }
}
