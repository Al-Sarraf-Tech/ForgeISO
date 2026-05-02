use std::path::Path;

use crate::error::{EngineError, EngineResult};
use crate::events::{EngineEvent, EventPhase};

use super::{DiffEntry, ForgeIsoEngine, IsoDiff};

impl ForgeIsoEngine {
    /// Compare the file trees of two ISOs and return an [`IsoDiff`] summarising
    /// added, removed, modified (size-changed), and unchanged entries.
    ///
    /// Uses `xorriso -find / -type f -exec lsdl` to extract per-file sizes;
    /// falls back to path-only listing (all sizes reported as 0) when the lsdl
    /// output is empty. Returns an error if xorriso is not found.
    #[allow(clippy::unused_async)] // async kept for API consistency
    pub async fn diff_isos(&self, base: &Path, target: &Path) -> EngineResult<IsoDiff> {
        self.emit(EngineEvent::info(
            EventPhase::Diff,
            "comparing ISO filesystems",
        ));

        let base_files = get_iso_file_list(base)?;
        let target_files = get_iso_file_list(target)?;

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged = 0;

        for (path, target_size) in &target_files {
            if let Some(base_size) = base_files.get(path) {
                if base_size == target_size {
                    unchanged += 1;
                } else {
                    modified.push(DiffEntry {
                        path: path.clone(),
                        base_size: *base_size,
                        target_size: *target_size,
                    });
                }
            } else {
                added.push(path.clone());
            }
        }

        for path in base_files.keys() {
            if !target_files.contains_key(path) {
                removed.push(path.clone());
            }
        }

        self.emit(EngineEvent::info(
            EventPhase::Diff,
            format!(
                "diff: {} added, {} removed, {} modified, {} unchanged",
                added.len(),
                removed.len(),
                modified.len(),
                unchanged
            ),
        ));
        self.emit(EngineEvent::info(
            EventPhase::Complete,
            "ISO diff completed",
        ));

        Ok(IsoDiff {
            added,
            removed,
            modified,
            unchanged,
        })
    }
}

/// List all files in an ISO with their sizes.
///
/// Tries two methods in order:
///  1. `lsdl` exec action (no arg)  — all xorriso versions >= 1.5.4
///     Output: `perms nlinks uid gid size month day time/year 'path'`
///     Note: `.` and `{}` path tokens are NOT accepted by xorriso 1.5.6 `-find -exec`
///  2. plain `-find / -type f`      — last resort, paths only with size = 0
fn get_iso_file_list(iso_path: &Path) -> EngineResult<std::collections::HashMap<String, u64>> {
    use std::process::Command;

    let iso_str = iso_path.to_string_lossy();

    // -- Method 1: lsdl exec (works on xorriso 1.5.4-1.5.7+) -----------------
    // `-exec lsdl` with NO path argument applies lsdl to each found file.
    // xorriso 1.5.6 rejects `.` and `{}` after the exec action name.
    if let Ok(out) = Command::new("xorriso")
        .args([
            "-indev", &iso_str, "-find", "/", "-type", "f", "-exec", "lsdl",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        let files = parse_lsdl_output(&text);
        if !files.is_empty() {
            return Ok(files);
        }
    }

    // -- Method 2: paths only, no sizes (minimum viable diff) -----------------
    let out = Command::new("xorriso")
        .args(["-indev", &iso_str, "-find", "/", "-type", "f"])
        .output()
        .map_err(|e| EngineError::Runtime(format!("xorriso not found: {e}")))?;

    if !out.status.success() {
        return Err(EngineError::Runtime(format!(
            "xorriso failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with('/') || l.starts_with("'/"))
        .map(|l| {
            let path = l.trim_matches('\'').to_string();
            (path, 0u64)
        })
        .collect())
}

fn parse_lsdl_output(text: &str) -> std::collections::HashMap<String, u64> {
    // xorriso -find / -type f -exec lsdl output format (1.5.x):
    // `-rwxr--r--    1 1000     1000       966664 Aug 13  2024 '/EFI/boot/bootx64.efi'`
    // Fields: [0]perms [1]nlinks [2]uid [3]gid [4]size [5]month [6]day [7]year/time [8+]'path'
    let mut files = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // File entries start with permission chars (-, d, l, etc.)
        let first = line.chars().next().unwrap_or(' ');
        if !matches!(first, '-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Need at least: perms nlinks uid gid size month day time/year path
        if parts.len() >= 9 {
            if let Ok(size) = parts[4].parse::<u64>() {
                // Path is the last fields joined, strip surrounding single quotes
                let raw_path = parts[8..].join(" ");
                let path = raw_path.trim_matches('\'').to_string();
                if path.starts_with('/') {
                    files.insert(path, size);
                }
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lsdl_output_extracts_path_and_size() {
        let text = "\
-rwxr--r--    1 1000     1000       966664 Aug 13  2024 '/EFI/boot/bootx64.efi'
-rw-r--r--    1 root     root           42 Jan 01  2025 '/casper/initrd'
";
        let map = parse_lsdl_output(text);
        assert_eq!(map.get("/EFI/boot/bootx64.efi").copied(), Some(966664));
        assert_eq!(map.get("/casper/initrd").copied(), Some(42));
    }

    #[test]
    fn parse_lsdl_output_skips_blank_and_non_file_lines() {
        // Lines beginning with anything other than perm chars must be ignored.
        let text = "\
some-summary-line
total: 5

-rw-r--r--    1 a b 10 Jan 1 2024 '/x'
";
        let map = parse_lsdl_output(text);
        assert_eq!(map.len(), 1, "only the file line should parse");
        assert_eq!(map.get("/x").copied(), Some(10));
    }

    #[test]
    fn parse_lsdl_output_handles_paths_with_spaces() {
        let text = "-rw-r--r-- 1 a b 99 Jan 1 2024 '/dir with space/file name.txt'\n";
        let map = parse_lsdl_output(text);
        assert_eq!(map.get("/dir with space/file name.txt").copied(), Some(99));
    }

    #[test]
    fn parse_lsdl_output_returns_empty_for_empty_input() {
        assert!(parse_lsdl_output("").is_empty());
    }

    #[test]
    fn parse_lsdl_output_skips_relative_paths() {
        // Without a leading slash the entry must be discarded — IsoDiff
        // requires absolute paths to compare across base/target trees.
        let text = "-rw-r--r-- 1 a b 1 Jan 1 2024 'relative/path.txt'\n";
        let map = parse_lsdl_output(text);
        assert!(map.is_empty(), "relative paths must not be inserted");
    }

    #[test]
    fn parse_lsdl_output_skips_unparseable_size_field() {
        // size column is not a number → discard the row
        let text = "-rw-r--r-- 1 a b NOT_A_NUMBER Jan 1 2024 '/bad-size'\n";
        let map = parse_lsdl_output(text);
        assert!(map.is_empty());
    }
}
