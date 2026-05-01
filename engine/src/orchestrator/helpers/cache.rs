use std::path::PathBuf;

use crate::error::{EngineError, EngineResult};

pub fn default_cache_root() -> EngineResult<PathBuf> {
    if let Ok(path) = std::env::var("FORGEISO_CACHE_DIR") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }

    // XDG-compliant default: ~/.cache/forgeiso — avoids tmpfs quota issues and
    // the world-writable /tmp directory (which is susceptible to cache-poisoning
    // attacks on shared hosts).  If $HOME is unavailable the caller must provide
    // an explicit cache_dir instead of silently falling back to /tmp.
    let home = std::env::var("HOME").map_err(|_| {
        EngineError::InvalidConfig(
            "$HOME is not set; cannot determine default cache directory. \
             Set $HOME or provide an explicit --cache-dir"
                .to_string(),
        )
    })?;
    let path = PathBuf::from(home).join(".cache").join("forgeiso");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn cache_subdir(name: &str) -> EngineResult<PathBuf> {
    let path = default_cache_root()?.join(name);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}
