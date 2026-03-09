use crate::state::{
    BuildResult, DoctorReport, Iso9660Compliance, IsoDiff, IsoMetadata, PickTarget, VerifyResult,
};

// ── Messages sent from worker threads back to the UI ──────────────────────────

pub enum WorkerMsg {
    // Engine progress events
    EngineEvent {
        phase: String,
        message: String,
        percent: Option<f32>,
        is_error: bool,
        is_warn: bool,
    },
    // Operation results
    InjectOk(Box<BuildResult>),
    VerifyOk(Box<VerifyResult>),
    Iso9660Ok(Box<Iso9660Compliance>),
    DiffOk(Box<IsoDiff>),
    BuildOk(Box<BuildResult>),
    InspectOk(Box<IsoMetadata>),
    /// Background ISO detection triggered after a file is picked for the inject
    /// source — used to auto-populate the distro/label fields without touching
    /// the build-tab inspect result.
    IsoDetected(Box<IsoMetadata>),
    DoctorOk(Box<DoctorReport>),
    ScanOk,
    TestOk,
    ReportOk(String),
    // File picker
    FilePicked {
        target: PickTarget,
        path: String,
    },
    // Error from any operation
    OpError(String),
    // Marks the end of any long-running operation (clears running flag)
    Done,
}

/// Spawn zenity file picker on a blocking thread, sending result back to UI.
pub fn pick_iso(target: PickTarget, tx: std::sync::mpsc::Sender<WorkerMsg>) {
    std::thread::spawn(move || {
        let result = std::process::Command::new("zenity")
            .args([
                "--file-selection",
                "--title=Select ISO Image",
                "--file-filter=ISO Images (*.iso)|*.iso",
            ])
            .output();
        handle_zenity(result, target, &tx);
    });
}

pub fn pick_folder(target: PickTarget, tx: std::sync::mpsc::Sender<WorkerMsg>) {
    std::thread::spawn(move || {
        let result = std::process::Command::new("zenity")
            .args(["--file-selection", "--directory", "--title=Select Folder"])
            .output();
        handle_zenity(result, target, &tx);
    });
}

pub fn pick_file(target: PickTarget, tx: std::sync::mpsc::Sender<WorkerMsg>) {
    std::thread::spawn(move || {
        let result = std::process::Command::new("zenity")
            .args(["--file-selection", "--title=Select File"])
            .output();
        handle_zenity(result, target, &tx);
    });
}

fn handle_zenity(
    result: std::io::Result<std::process::Output>,
    target: PickTarget,
    tx: &std::sync::mpsc::Sender<WorkerMsg>,
) {
    match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // zenity not installed — surface a clear error rather than silently
            // doing nothing, so users on headless or minimal systems understand
            // why Browse did nothing and can paste the path manually instead.
            let _ = tx.send(WorkerMsg::OpError(
                "File picker (zenity) not found — install zenity or paste the path manually".into(),
            ));
        }
        Err(e) => {
            let _ = tx.send(WorkerMsg::OpError(format!("File picker error: {e}")));
        }
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path.is_empty() {
                // zenity returned success but empty path — treat as cancel
                let _ = tx.send(WorkerMsg::Done);
            } else {
                let _ = tx.send(WorkerMsg::FilePicked { target, path });
            }
        }
        Ok(_) => {
            // Non-zero exit = user cancelled the dialog; no action needed.
            let _ = tx.send(WorkerMsg::Done);
        }
    }
}
