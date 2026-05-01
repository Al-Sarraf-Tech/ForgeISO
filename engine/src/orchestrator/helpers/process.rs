use std::path::Path;

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

pub(in crate::orchestrator) async fn run_command_capture_async(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let program = program.to_string();
    let args = args.to_vec();
    let cwd = cwd.map(Path::to_path_buf);
    tokio::task::spawn_blocking(move || run_command_capture(&program, &args, cwd.as_deref()))
        .await
        .map_err(|e| EngineError::Runtime(format!("failed to join blocking task: {e}")))?
}

pub(in crate::orchestrator) async fn run_command_lossy_async(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> EngineResult<CommandOutput> {
    let program = program.to_string();
    let args = args.to_vec();
    let cwd = cwd.map(Path::to_path_buf);
    tokio::task::spawn_blocking(move || run_command_lossy(&program, &args, cwd.as_deref()))
        .await
        .map_err(|e| EngineError::Runtime(format!("failed to join blocking task: {e}")))?
}
