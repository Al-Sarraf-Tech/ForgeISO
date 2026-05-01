use std::path::PathBuf;

use crate::error::{EngineError, EngineResult};

pub(in crate::orchestrator) fn ensure_linux_host() -> EngineResult<()> {
    if std::env::consts::OS != "linux" {
        return Err(EngineError::MissingTool(
            "ForgeISO local build/test is supported only on Linux hosts".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::orchestrator) fn require_tools(tools: &[&str]) -> EngineResult<()> {
    let missing = tools
        .iter()
        .filter(|tool| which::which(tool).is_err())
        .copied()
        .collect::<Vec<_>>();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(EngineError::MissingTool(format!(
            "missing local tools: {}",
            missing.join(", ")
        )))
    }
}

pub(in crate::orchestrator) fn ovmf_path() -> EngineResult<PathBuf> {
    for candidate in [
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/edk2/x64/OVMF_CODE.fd",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(EngineError::MissingTool(
        "OVMF firmware is required for UEFI smoke tests".to_string(),
    ))
}
