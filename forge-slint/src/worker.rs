use crate::{AppState, AppWindow};
/// Spawn zenity file pickers that deliver results via `slint::invoke_from_event_loop`.
/// Spawns zenity file pickers and delivers results via `slint::invoke_from_event_loop`.
use slint::{ComponentHandle, Weak};
use std::ffi::OsStr;
use std::process::Output;

pub fn pick_iso(win: Weak<AppWindow>, on_picked: impl Fn(AppWindow, String) + Send + 'static) {
    std::thread::spawn(move || {
        if !has_graphical_session() {
            report_picker_error(
                win,
                "File picker unavailable without a graphical session — type the path manually",
            );
            return;
        }
        let result = run_picker(PickerKind::Iso);
        handle_picker_result(result, win, on_picked);
    });
}

pub fn pick_folder(win: Weak<AppWindow>, on_picked: impl Fn(AppWindow, String) + Send + 'static) {
    std::thread::spawn(move || {
        if !has_graphical_session() {
            report_picker_error(
                win,
                "Folder picker unavailable without a graphical session — type the path manually",
            );
            return;
        }
        let result = run_picker(PickerKind::Folder);
        handle_picker_result(result, win, on_picked);
    });
}

#[derive(Clone, Copy)]
enum PickerKind {
    Iso,
    Folder,
}

fn run_picker(kind: PickerKind) -> std::io::Result<Output> {
    for program in picker_programs() {
        let result = match (program, kind) {
            ("zenity", PickerKind::Iso) => std::process::Command::new("zenity")
                .args([
                    "--file-selection",
                    "--title=Select ISO Image",
                    "--file-filter=ISO Images (*.iso)",
                    "--file-filter=*.iso",
                ])
                .output(),
            ("zenity", PickerKind::Folder) => std::process::Command::new("zenity")
                .args(["--file-selection", "--directory", "--title=Select Folder"])
                .output(),
            ("kdialog", PickerKind::Iso) => std::process::Command::new("kdialog")
                .args([
                    "--getopenfilename",
                    "",
                    "*.iso|ISO Images (*.iso)",
                    "--title",
                    "Select ISO Image",
                ])
                .output(),
            ("kdialog", PickerKind::Folder) => std::process::Command::new("kdialog")
                .args(["--getexistingdirectory", "", "--title", "Select Folder"])
                .output(),
            _ => continue,
        };

        match result {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            other => return other,
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no supported file picker found",
    ))
}

fn handle_picker_result(
    result: std::io::Result<std::process::Output>,
    win: Weak<AppWindow>,
    on_picked: impl Fn(AppWindow, String) + Send + 'static,
) {
    match interpret_picker_result(result) {
        PickerOutcome::Picked(path) => {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    on_picked(w, path);
                }
            });
        }
        PickerOutcome::Cancelled => {}
        PickerOutcome::Error(msg) => report_picker_error(win, &msg),
    }
}

enum PickerOutcome {
    Picked(String),
    Cancelled,
    Error(String),
}

fn interpret_picker_result(result: std::io::Result<Output>) -> PickerOutcome {
    match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => PickerOutcome::Error(
            "No graphical file picker found — install zenity or kdialog, or type the path manually"
                .to_string(),
        ),
        Err(e) => PickerOutcome::Error(format!("File picker error: {e}")),
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path.is_empty() {
                PickerOutcome::Cancelled
            } else {
                PickerOutcome::Picked(path)
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if out.status.code() == Some(1) && stderr.is_empty() {
                PickerOutcome::Cancelled
            } else if stderr.is_empty() {
                PickerOutcome::Error("File picker failed unexpectedly.".to_string())
            } else {
                PickerOutcome::Error(format!("File picker failed: {stderr}"))
            }
        }
    }
}

fn report_picker_error(win: Weak<AppWindow>, msg: &str) {
    let msg = msg.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = win.upgrade() {
            let gs = w.global::<AppState>();
            gs.set_status_text(msg.into());
            gs.set_status_is_error(true);
        }
    });
}

fn has_graphical_session() -> bool {
    has_graphical_session_from(
        std::env::var_os("DISPLAY").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
    )
}

fn has_graphical_session_from(display: Option<&OsStr>, wayland: Option<&OsStr>) -> bool {
    display.is_some_and(|value| !value.is_empty()) || wayland.is_some_and(|value| !value.is_empty())
}

fn picker_programs() -> [&'static str; 2] {
    ["zenity", "kdialog"]
}

#[cfg(test)]
mod tests {
    use super::{
        has_graphical_session_from, interpret_picker_result, picker_programs, PickerOutcome,
    };
    use std::ffi::OsStr;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    fn output(code: i32, stdout: &[u8], stderr: &[u8]) -> Output {
        Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        }
    }

    #[test]
    fn graphical_session_detects_display_or_wayland() {
        assert!(has_graphical_session_from(Some(OsStr::new(":0")), None));
        assert!(has_graphical_session_from(
            None,
            Some(OsStr::new("wayland-0"))
        ));
        assert!(!has_graphical_session_from(None, None));
        assert!(!has_graphical_session_from(
            Some(OsStr::new("")),
            Some(OsStr::new("")),
        ));
    }

    #[test]
    fn picker_programs_prefers_zenity_then_kdialog() {
        assert_eq!(picker_programs(), ["zenity", "kdialog"]);
    }

    #[test]
    fn picker_cancel_is_not_reported_as_error() {
        let outcome = interpret_picker_result(Ok(output(1, b"", b"")));
        assert!(matches!(outcome, PickerOutcome::Cancelled));
    }

    #[test]
    fn picker_stderr_is_exposed_to_user() {
        let outcome =
            interpret_picker_result(Ok(output(1, b"", b"Gtk-WARNING **: cannot open display")));
        assert!(
            matches!(outcome, PickerOutcome::Error(msg) if msg.contains("cannot open display"))
        );
    }
}
