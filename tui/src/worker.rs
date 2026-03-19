use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use forgeiso_engine::ForgeIsoEngine;
use tokio::sync::mpsc;

use crate::state::{App, LogLevel, WorkerMsg};

impl App {
    pub(crate) fn spawn_inject(
        &mut self,
        engine: Arc<ForgeIsoEngine>,
        tx: mpsc::UnboundedSender<WorkerMsg>,
    ) {
        let cfg = match self.build_inject_config() {
            Ok(c) => c,
            Err(e) => {
                self.status = format!("Error: {e}");
                return;
            }
        };
        self.busy = true;
        self.progress.build_done = false;
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.build_artifact = None;
        self.build_sha256 = None;
        self.verify_result = None;
        self.iso9660_result = None;
        self.status = "Building ISO...".into();
        let out_dir = PathBuf::from(&self.output_dir);

        // Subscribe to engine events in the spawned task.
        let mut rx = engine.subscribe();
        let tx2 = tx.clone();
        tokio::spawn(async move {
            loop {
                match rx.try_recv() {
                    Ok(ev) => {
                        let level = match ev.level {
                            forgeiso_engine::EventLevel::Warn => LogLevel::Warn,
                            forgeiso_engine::EventLevel::Error => LogLevel::Error,
                            _ => LogLevel::Info,
                        };
                        let _ = tx2.send(WorkerMsg::EngineEvent(
                            format!("[{:?}] {}", ev.phase, ev.message),
                            level,
                        ));
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(_) => break,
                }
            }
        });

        tokio::spawn(async move {
            let msg = match engine.inject_autoinstall(&cfg, &out_dir).await {
                Ok(r) => WorkerMsg::InjectOk(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("Build failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn spawn_verify(
        &mut self,
        engine: Arc<ForgeIsoEngine>,
        tx: mpsc::UnboundedSender<WorkerMsg>,
    ) {
        let source = self.verify_source.trim().to_string();
        if source.is_empty() {
            self.status = "Enter an ISO path to verify".into();
            return;
        }
        self.busy = true;
        self.progress.verify_done = false;
        self.verify_result = None;
        self.status = "Verifying checksum...".into();
        tokio::spawn(async move {
            let msg = match engine.verify(&source, None).await {
                Ok(r) => WorkerMsg::VerifyOk(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("Verify failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn spawn_iso9660(
        &mut self,
        engine: Arc<ForgeIsoEngine>,
        tx: mpsc::UnboundedSender<WorkerMsg>,
    ) {
        let source = self.verify_source.trim().to_string();
        if source.is_empty() {
            self.status = "Enter an ISO path to validate".into();
            return;
        }
        self.busy = true;
        self.progress.iso9660_done = false;
        self.iso9660_result = None;
        self.status = "Validating ISO-9660...".into();
        tokio::spawn(async move {
            let msg = match engine.validate_iso9660(&source).await {
                Ok(r) => WorkerMsg::Iso9660Ok(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("ISO-9660 validation failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }
}
