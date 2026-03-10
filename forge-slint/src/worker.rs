use crate::AppWindow;
/// Spawn zenity file pickers that deliver results via `slint::invoke_from_event_loop`.
/// This is the Slint-native equivalent of the mpsc-based worker in forge-gui.
use slint::Weak;

pub fn pick_iso(win: Weak<AppWindow>, on_picked: impl Fn(AppWindow, String) + Send + 'static) {
    std::thread::spawn(move || {
        let result = std::process::Command::new("zenity")
            .args([
                "--file-selection",
                "--title=Select ISO Image",
                "--file-filter=ISO Images (*.iso)",
                "--file-filter=*.iso",
            ])
            .output();
        handle_zenity(result, win, on_picked);
    });
}

pub fn pick_folder(win: Weak<AppWindow>, on_picked: impl Fn(AppWindow, String) + Send + 'static) {
    std::thread::spawn(move || {
        let result = std::process::Command::new("zenity")
            .args(["--file-selection", "--directory", "--title=Select Folder"])
            .output();
        handle_zenity(result, win, on_picked);
    });
}

fn handle_zenity(
    result: std::io::Result<std::process::Output>,
    win: Weak<AppWindow>,
    on_picked: impl Fn(AppWindow, String) + Send + 'static,
) {
    match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    w.set_status_text(
                        "File picker (zenity) not found — install zenity or type the path directly"
                            .into(),
                    );
                    w.set_status_is_error(true);
                }
            });
        }
        Err(e) => {
            let msg = format!("File picker error: {e}");
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    w.set_status_text(msg.into());
                    w.set_status_is_error(true);
                }
            });
        }
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if path.is_empty() {
                // user cancelled
            } else {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win.upgrade() {
                        on_picked(w, path);
                    }
                });
            }
        }
        Ok(_) => { /* user cancelled — no action needed */ }
    }
}
