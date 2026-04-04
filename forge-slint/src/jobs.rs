use std::path::PathBuf;
use std::sync::Arc;

use forgeiso_engine::{Distro, EventLevel};
use sha2::{Digest, Sha256};
use slint::{ComponentHandle, Weak};

use crate::app::{ForgeApp, APP};
use crate::state::opt;
use crate::{clear_build_results, AppState, AppWindow, FormState};

// ── spawn_inject ─────────────────────────────────────────────────────────────

impl ForgeApp {
    pub fn spawn_inject(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };

        if w.global::<AppState>().get_job_running() {
            return;
        }

        let (inject, cfg) = match self.collect_inject_config() {
            Ok(values) => values,
            Err(msg) => {
                self.set_status_err(msg);
                return;
            }
        };

        // Abort stale tasks
        if let Some(h) = self.detect_task.take() {
            h.abort();
        }
        if let Some(h) = self.sha256_task.take() {
            h.abort();
        }

        // Reset build/check state while preserving completed configuration.
        clear_build_results(&w);

        self.start_job("Injecting\u{2026}");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();
        let out_dir = PathBuf::from(&inject.output_dir);

        self.current_task = Some(self.rt.spawn(async move {
            match engine.inject_autoinstall(&cfg, &out_dir).await {
                Ok(result) => {
                    let artifact = result
                        .artifacts
                        .first()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();

                    let art2 = artifact.clone();
                    let win3 = win2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            let gs = w.global::<AppState>();
                            gs.set_job_running(false);
                            gs.set_job_phase("".into());
                            gs.set_step3_done(true);
                            gs.set_artifact_path(artifact.clone().into());
                            gs.set_verify_source(artifact.into());
                            gs.set_current_step(3);
                            gs.set_status_text(
                                "ISO ready \u{2014} optional checks are available if you want them"
                                    .into(),
                            );
                            gs.set_status_is_error(false);
                        }
                    });

                    // Background SHA-256
                    if !art2.is_empty() {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win3.upgrade() {
                                spawn_sha256(w.as_weak(), art2);
                            }
                        });
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            let gs = w.global::<AppState>();
                            gs.set_job_running(false);
                            gs.set_job_phase("".into());
                            gs.set_status_text(
                                msg.as_str().chars().take(200).collect::<String>().into(),
                            );
                            gs.set_status_is_error(true);
                        }
                    });
                }
            }
        }));
    }

    // ── spawn_verify ─────────────────────────────────────────────────────────

    pub fn spawn_verify(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };
        let gs = w.global::<AppState>();
        if gs.get_job_running() {
            return;
        }
        // Abort any previous verify
        if let Some(h) = self.current_task.take() {
            h.abort();
        }

        let source: String = gs.get_verify_source().into();
        if source.trim().is_empty() {
            self.set_status_err("Source ISO is required for verification");
            return;
        }
        let sums: String = gs.get_sums_url().into();
        let sums_opt = opt(&sums);

        gs.set_verify_done(false);
        gs.set_verify_matched(false);
        gs.set_verify_hash_display("".into());
        self.start_job("Verifying checksum\u{2026}");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.current_task = Some(self.rt.spawn(async move {
            match engine.verify(&source, sums_opt.as_deref()).await {
                Ok(r) => {
                    let matched = r.matched;
                    let hash = r.actual.clone();
                    let display = format!(
                        "{}{}",
                        if matched {
                            "Match: "
                        } else {
                            "Mismatch \u{2014} actual: "
                        },
                        hash
                    );
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            let gs = w.global::<AppState>();
                            gs.set_job_running(false);
                            gs.set_job_phase("".into());
                            gs.set_verify_done(true);
                            gs.set_verify_matched(matched);
                            gs.set_verify_hash_display(display.into());
                            gs.set_step3_done(true);
                            gs.set_status_text(
                                if matched {
                                    "Integrity verified \u{2713}"
                                } else {
                                    "Checksum mismatch"
                                }
                                .into(),
                            );
                            gs.set_status_is_error(!matched);
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            let gs = w.global::<AppState>();
                            gs.set_job_running(false);
                            gs.set_job_phase("".into());
                            gs.set_status_text(
                                msg.as_str().chars().take(200).collect::<String>().into(),
                            );
                            gs.set_status_is_error(true);
                        }
                    });
                }
            }
        }));
    }

    // ── spawn_iso9660 ────────────────────────────────────────────────────────

    pub fn spawn_iso9660(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };
        let gs = w.global::<AppState>();
        if gs.get_job_running() {
            return;
        }
        if let Some(h) = self.current_task.take() {
            h.abort();
        }

        let source: String = gs.get_verify_source().into();
        if source.trim().is_empty() {
            self.set_status_err("Source ISO is required");
            return;
        }
        gs.set_iso9660_done(false);
        gs.set_iso9660_compliant(false);
        gs.set_iso9660_boot_bios(false);
        gs.set_iso9660_boot_uefi(false);
        gs.set_iso9660_volume_id("".into());
        self.start_job("Validating ISO-9660\u{2026}");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.current_task = Some(self.rt.spawn(async move {
            match engine.validate_iso9660(&source).await {
                Ok(r) => {
                    let compliant = r.compliant;
                    let bios = r.boot_bios;
                    let uefi = r.boot_uefi;
                    let vol = r.volume_id.clone().unwrap_or_default();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            let gs = w.global::<AppState>();
                            gs.set_job_running(false);
                            gs.set_job_phase("".into());
                            gs.set_iso9660_done(true);
                            gs.set_iso9660_compliant(compliant);
                            gs.set_iso9660_boot_bios(bios);
                            gs.set_iso9660_boot_uefi(uefi);
                            gs.set_iso9660_volume_id(vol.into());
                            gs.set_status_text(
                                if compliant {
                                    "ISO-9660 compliant \u{2713}"
                                } else {
                                    "ISO-9660 issues found"
                                }
                                .into(),
                            );
                            gs.set_status_is_error(!compliant);
                        }
                    });
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win2.upgrade() {
                            let gs = w.global::<AppState>();
                            gs.set_job_running(false);
                            gs.set_job_phase("".into());
                            gs.set_status_text(
                                msg.as_str().chars().take(200).collect::<String>().into(),
                            );
                            gs.set_status_is_error(true);
                        }
                    });
                }
            }
        }));
    }

    // ── spawn_doctor ─────────────────────────────────────────────────────────

    pub fn spawn_doctor(&mut self) {
        let w = match self.win.upgrade() {
            Some(w) => w,
            None => return,
        };
        let gs = w.global::<AppState>();
        if gs.get_job_running() {
            return;
        }
        if let Some(h) = self.current_task.take() {
            h.abort();
        }

        gs.set_doctor_loading(true);
        gs.set_doctor_text("".into());
        self.start_job("Checking dependencies\u{2026}");

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.current_task = Some(self.rt.spawn(async move {
            let report = engine.doctor().await;
            // Build a human-readable text from BTreeMap<tool, ok>.
            let mut lines_out = Vec::new();
            for (tool, ok) in &report.tooling {
                let mark = if *ok { "\u{2713}" } else { "\u{2717}" };
                lines_out.push(format!("  {mark}  {tool}"));
            }
            if !report.warnings.is_empty() {
                lines_out.push(String::new());
                for w in &report.warnings {
                    lines_out.push(format!("  \u{26A0}  {w}"));
                }
            }
            let text = lines_out.join("\n");
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win2.upgrade() {
                    let gs = w.global::<AppState>();
                    gs.set_job_running(false);
                    gs.set_job_phase("".into());
                    gs.set_doctor_loading(false);
                    gs.set_doctor_text(text.into());
                }
            });
        }));
    }

    // ── spawn_detect_iso ─────────────────────────────────────────────────────

    pub fn spawn_detect_iso(&mut self, path: String) {
        // Abort any existing detection
        if let Some(h) = self.detect_task.take() {
            h.abort();
        }

        let engine = Arc::clone(&self.engine);
        let win2 = self.win.clone();

        self.detect_task = Some(self.rt.spawn(async move {
            if let Ok(meta) = engine.inspect_source(&path, None).await {
                let distro_str = match meta.distro {
                    Some(Distro::Ubuntu) => "ubuntu",
                    Some(Distro::Fedora) => "fedora",
                    Some(Distro::Arch) => "arch",
                    Some(Distro::Mint) => "mint",
                    _ => "",
                };
                let label = meta.volume_id.clone().unwrap_or_default();
                let distro = distro_str.to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win2.upgrade() {
                        let fs = w.global::<FormState>();
                        if !distro.is_empty() {
                            fs.set_distro(distro.clone().into());
                            fs.set_detected_distro(
                                match distro.as_str() {
                                    "fedora" => "Fedora / RHEL",
                                    "arch" => "Arch Linux",
                                    "mint" => "Linux Mint",
                                    _ => "Ubuntu",
                                }
                                .into(),
                            );
                        }
                        if !label.is_empty() && fs.get_output_label().is_empty() {
                            fs.set_output_label(label.into());
                        }
                    }
                });
            }
        }));
    }

    // ── subscribe_events ─────────────────────────────────────────────────────

    pub fn subscribe_events(&self) -> tokio::task::JoinHandle<()> {
        let mut rx = self.engine.subscribe();
        self.rt.spawn(async move {
            while let Ok(ev) = rx.recv().await {
                let phase = format!("{:?}", ev.phase);
                let msg = ev.message.clone();
                let level = match ev.level {
                    EventLevel::Error => 2i32,
                    EventLevel::Warn => 1i32,
                    _ => 0i32,
                };
                let pct = ev.percent;
                let _ = slint::invoke_from_event_loop(move || {
                    APP.with(|cell| {
                        if let Some(rc) = cell.borrow().as_ref() {
                            rc.borrow_mut().push_log(&phase, &msg, level, pct);
                        }
                    });
                });
            }
        })
    }
}

// ── SHA-256 background task ──────────────────────────────────────────────────

pub fn spawn_sha256(win: Weak<AppWindow>, path: String) {
    std::thread::spawn(move || {
        let hash = compute_sha256(&path);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = win.upgrade() {
                w.global::<AppState>().set_artifact_sha256(hash.into());
            }
        });
    });
}

fn compute_sha256(path: &str) -> String {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    if std::io::copy(&mut file, &mut hasher).is_err() {
        return String::new();
    }
    format!("{:x}", hasher.finalize())
}

/// Re-hash the output ISO and compare to the stored build hash.
/// Confirms the file on disk matches what was written (write integrity check).
pub fn spawn_verify_output(win: Weak<AppWindow>, path: String, expected_hash: String) {
    std::thread::spawn(move || {
        let actual = compute_sha256(&path);
        let matched = !actual.is_empty() && actual == expected_hash;
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = win.upgrade() {
                let gs = w.global::<AppState>();
                gs.set_output_verified(true);
                gs.set_output_verify_matched(matched);
            }
        });
    });
}
