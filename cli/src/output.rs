use forgeiso_engine::{EventPhase, ForgeIsoEngine};
use tokio::task::JoinHandle;

/// Subscribe to engine broadcast events and spawn a task that prints them to stderr.
/// Returns the join handle for the spawned task.
pub fn spawn_event_subscriber(engine: &ForgeIsoEngine) -> JoinHandle<()> {
    let mut rx = engine.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event.phase {
                EventPhase::Download => {
                    eprint!("\r[Download] {:<40}", event.message);
                    let _ = std::io::Write::flush(&mut std::io::stderr());
                }
                _ => {
                    eprintln!("[{:?}] {}", event.phase, event.message);
                }
            }
        }
    })
}
