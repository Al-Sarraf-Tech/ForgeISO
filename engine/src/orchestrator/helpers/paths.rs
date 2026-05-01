use std::path::Path;

use walkdir::WalkDir;

use crate::error::{EngineError, EngineResult};

pub(in crate::orchestrator) fn is_squashfs_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".squashfs") || lower.ends_with(".sfs") || lower.ends_with(".erofs")
}

pub(in crate::orchestrator) fn download_filename(url: &str) -> String {
    let fallback = format!("download-{}.iso", chrono::Utc::now().timestamp());
    // Strip query string and fragment before extracting the path basename so
    // that URLs like ".../ubuntu.iso?token=abc" produce "ubuntu.iso" rather
    // than the mangled "ubuntu.iso-token-abc".
    let without_query = url.split_once('?').map_or(url, |(p, _)| p);
    let path_only = without_query
        .split_once('#')
        .map_or(without_query, |(p, _)| p);
    path_only
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(sanitize_filename)
        .filter(|segment| !segment.is_empty())
        .unwrap_or(fallback)
}

pub(in crate::orchestrator) fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub(in crate::orchestrator) fn copy_dir_contents(from: &Path, to: &Path) -> EngineResult<()> {
    for entry in WalkDir::new(from).into_iter().filter_map(Result::ok) {
        let relative = entry.path().strip_prefix(from).map_err(|e| {
            EngineError::Runtime(format!("failed to compute relative overlay path: {e}"))
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Recursively grant user-write permission before removal so files extracted
/// from ISOs (which may carry read-only permissions) can be deleted.
pub(in crate::orchestrator) fn remove_dir_all_force(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let Ok(meta) = entry.metadata() else { continue };
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(entry.path(), perms);
    }
    std::fs::remove_dir_all(path)
}

pub(in crate::orchestrator) fn chmod_recursive_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let Ok(meta) = entry.metadata() else { continue };
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o700);
        let _ = std::fs::set_permissions(entry.path(), perms);
    }
}
