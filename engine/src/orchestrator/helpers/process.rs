use std::path::Path;
use std::process::Stdio;

use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use crate::error::{EngineError, EngineResult};

use crate::orchestrator::CommandOutput;

pub fn run_command_capture(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

    if !output.status.success() {
        return Err(EngineError::Runtime(format!(
            "{program} failed with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Like `run_command_capture` but tolerates non-zero exit codes (e.g. unsquashfs
/// returning exit 2 for device-node warnings when not running as root).
pub fn run_command_lossy(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Async variant of [`run_command_lossy`]. Behaviourally identical to
/// [`run_command_lossy_async_cancellable`] called with `cancel = None`,
/// preserved as a thin wrapper so existing callers compile unchanged.
pub(in crate::orchestrator) async fn run_command_lossy_async(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    run_command_lossy_async_cancellable(program, args, cwd, None).await
}

/// Cancellation-aware variant of [`run_command_capture_async`].
///
/// When `cancel` is `Some(token)`, the running subprocess is killed and the
/// function returns [`EngineError::Cancelled`] as soon as the token signals
/// cancellation. When `cancel` is `None`, behaviour is identical to
/// [`run_command_capture_async`].
pub(in crate::orchestrator) async fn run_command_capture_async_cancellable(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    cancel: Option<CancellationToken>,
) -> EngineResult<CommandOutput> {
    let output = run_async_inner(program, args, cwd, cancel).await?;

    if !output.status.success() {
        return Err(EngineError::Runtime(format!(
            "{program} failed with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Cancellation-aware variant of [`run_command_lossy_async`].
pub(in crate::orchestrator) async fn run_command_lossy_async_cancellable(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    cancel: Option<CancellationToken>,
) -> EngineResult<CommandOutput> {
    let output = run_async_inner(program, args, cwd, cancel).await?;

    Ok(CommandOutput {
        program: program.to_string(),
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Shared async implementation. Spawns the subprocess via `tokio::process`
/// and races it against the cancellation token.
async fn run_async_inner(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    cancel: Option<CancellationToken>,
) -> EngineResult<std::process::Output> {
    let mut command = tokio::process::Command::new(program);
    command.args(args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    // Disabling kill-on-drop is the previous behaviour; we kill explicitly
    // on cancellation so the lifecycle is observable and we surface a
    // typed [`EngineError::Cancelled`] rather than a generic I/O error.
    command.kill_on_drop(false);

    let mut child = command
        .spawn()
        .map_err(|e| EngineError::Runtime(format!("failed to run {program}: {e}")))?;

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();

    let wait = async {
        let status = child
            .wait()
            .await
            .map_err(|e| EngineError::Runtime(format!("failed to await {program}: {e}")))?;
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        if let Some(mut s) = stdout.take() {
            let _ = s.read_to_end(&mut stdout_buf).await;
        }
        if let Some(mut s) = stderr.take() {
            let _ = s.read_to_end(&mut stderr_buf).await;
        }
        Ok::<_, EngineError>(std::process::Output {
            status,
            stdout: stdout_buf,
            stderr: stderr_buf,
        })
    };

    match cancel {
        None => wait.await,
        Some(token) => {
            tokio::select! {
                biased;
                _ = token.cancelled() => {
                    // Best-effort termination. start_kill is non-blocking;
                    // wait collects the zombie. Failure to kill is logged
                    // implicitly via the returned Cancelled error — we do
                    // not surface a different variant because the user's
                    // intent (cancel) is already known.
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    Err(EngineError::Cancelled)
                }
                output = wait => output,
            }
        }
    }
}
