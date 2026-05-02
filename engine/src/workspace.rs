//! Per-build scratch directories.
//!
//! Every [`crate::ForgeIsoEngine::build`] call creates a
//! [`Workspace`] under the user-supplied `out_dir`. Inside the
//! workspace the engine extracts the source ISO, injects autoinstall
//! files, repacks, and writes the final `.iso` plus a build report.
//!
//! Workspaces are cleaned up when the build completes successfully
//! unless [`crate::config::BuildConfig::keep_workdir`] is `true`. The
//! root directory uses a UUID suffix to avoid collisions when several
//! builds run in parallel.
//!
//! Path joining inside the workspace goes through [`Workspace::safe_join`],
//! which rejects traversal escapes (`..`) and absolute paths to
//! contain damage from a malformed config or compromised injection
//! source.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{EngineError, EngineResult};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub input: PathBuf,
    pub work: PathBuf,
    pub output: PathBuf,
    pub reports: PathBuf,
    pub scans: PathBuf,
    pub logs: PathBuf,
}

impl Workspace {
    pub fn create(base: &Path, run_name: &str) -> EngineResult<Self> {
        std::fs::create_dir_all(base)?;

        let sanitized_name = sanitize_run_name(run_name);
        let root = base.join(format!("{}-{}", sanitized_name, Uuid::new_v4()));
        let input = root.join("input");
        let work = root.join("work");
        let output = root.join("output");
        let reports = root.join("reports");
        let scans = root.join("scans");
        let logs = root.join("logs");

        for dir in [&root, &input, &work, &output, &reports, &scans, &logs] {
            std::fs::create_dir_all(dir)?;
        }

        Ok(Self {
            root,
            input,
            work,
            output,
            reports,
            scans,
            logs,
        })
    }

    pub fn safe_join(&self, relative: &Path) -> EngineResult<PathBuf> {
        safe_join(&self.root, relative)
    }
}

pub fn safe_join(root: &Path, candidate: &Path) -> EngineResult<PathBuf> {
    let root = root
        .canonicalize()
        .map_err(|e| EngineError::PathSafety(format!("canonicalize root failed: {e}")))?;

    let mut joined = root.clone();

    if candidate.is_absolute() {
        // canonicalize() fails for non-existent paths; do NOT fall back to the
        // raw string — a path like `/workspace/../../etc/passwd` would pass the
        // starts_with check as a raw string. Return an error instead.
        let absolute = candidate.canonicalize().map_err(|e| {
            EngineError::PathSafety(format!(
                "cannot resolve absolute path '{}': {e}",
                candidate.display()
            ))
        })?;
        if !absolute.starts_with(&root) {
            return Err(EngineError::PathSafety(format!(
                "path escapes workspace: {}",
                absolute.display()
            )));
        }
        joined = absolute;
    } else {
        for component in candidate.components() {
            use std::path::Component;
            match component {
                Component::CurDir => {}
                Component::Normal(seg) => joined.push(seg),
                Component::ParentDir => {
                    if !joined.pop() || !joined.starts_with(&root) {
                        return Err(EngineError::PathSafety(format!(
                            "path escapes workspace: {}",
                            candidate.display()
                        )));
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(EngineError::PathSafety(format!(
                        "invalid component in relative path: {}",
                        candidate.display()
                    )))
                }
            }
        }
    }

    if let Some(parent) = joined.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !joined.starts_with(&root) {
        return Err(EngineError::PathSafety(format!(
            "path escapes workspace: {}",
            joined.display()
        )));
    }

    Ok(joined)
}

fn sanitize_run_name(input: &str) -> String {
    let cleaned = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();

    cleaned.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn safe_join_rejects_parent_escape() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("ok")).expect("mk root");

        let escaped = safe_join(root, Path::new("../etc/passwd"));
        assert!(escaped.is_err());
    }

    #[test]
    fn safe_join_allows_child_path() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("ok")).expect("mk root");

        let child = safe_join(root, Path::new("ok/file.txt")).expect("safe path");
        assert!(child.starts_with(root));
    }

    #[test]
    fn workspace_create_makes_all_subdirectories() {
        let base = TempDir::new().expect("base dir");
        let ws = Workspace::create(base.path(), "my-build").expect("create");
        for sub in [
            &ws.input,
            &ws.work,
            &ws.output,
            &ws.reports,
            &ws.scans,
            &ws.logs,
        ] {
            assert!(sub.exists(), "subdir must be created: {}", sub.display());
        }
        assert!(ws.root.starts_with(base.path()));
    }

    #[test]
    fn workspace_create_sanitizes_run_name_in_root_segment() {
        let base = TempDir::new().expect("base dir");
        let ws = Workspace::create(base.path(), "my build/with spaces").expect("create");
        let name = ws
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .expect("dirname");
        // sanitize_run_name replaces non-alnum (except - and _) with '-'
        assert!(
            !name.contains(' '),
            "spaces must be sanitized in path component: {name}"
        );
        assert!(
            !name.contains('/'),
            "slashes must not appear in directory component: {name}"
        );
    }

    #[test]
    fn workspace_safe_join_delegates_to_module_function() {
        let base = TempDir::new().expect("base dir");
        let ws = Workspace::create(base.path(), "ws").expect("create");
        // Relative child must resolve under root
        let p = ws.safe_join(Path::new("input/sample.iso")).expect("safe");
        assert!(p.starts_with(&ws.root));
    }

    #[test]
    fn safe_join_rejects_absolute_path_outside_root() {
        let temp = TempDir::new().expect("dir");
        let root = temp.path();
        // /etc/hostname exists on every Linux box and is outside the temp root.
        let result = safe_join(root, Path::new("/etc/hostname"));
        assert!(matches!(result, Err(EngineError::PathSafety(_))));
    }

    #[test]
    fn safe_join_rejects_absolute_path_when_target_unresolvable() {
        let temp = TempDir::new().expect("dir");
        let root = temp.path();
        // Path that does not exist -> canonicalize fails -> PathSafety error.
        let result = safe_join(root, Path::new("/nonexistent/zxcvb-12345"));
        assert!(matches!(result, Err(EngineError::PathSafety(_))));
    }

    #[test]
    fn safe_join_handles_curdir_components() {
        let temp = TempDir::new().expect("dir");
        let root = temp.path();
        // ./file.txt should resolve to <root>/file.txt
        let p = safe_join(root, Path::new("./file.txt")).expect("safe");
        let canonical_root = root.canonicalize().expect("canonical");
        assert!(p.starts_with(&canonical_root));
        assert!(p.ends_with("file.txt"));
    }

    #[test]
    fn sanitize_run_name_strips_leading_and_trailing_dashes() {
        assert_eq!(sanitize_run_name("---hello---"), "hello");
        assert_eq!(sanitize_run_name("a/b/c"), "a-b-c");
        assert_eq!(sanitize_run_name("simple_run-1"), "simple_run-1");
    }
}
