use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("policy violation: {0}")]
    PolicyViolation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("tooling missing: {0}")]
    MissingTool(String),
    #[error("filesystem safety violation: {0}")]
    PathSafety(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("http error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

pub type EngineResult<T> = Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_config_error_displays_message() {
        let err = EngineError::InvalidConfig("bad hostname".to_string());
        assert_eq!(err.to_string(), "invalid config: bad hostname");
    }

    #[test]
    fn runtime_error_displays_message() {
        let err = EngineError::Runtime("xorriso failed".to_string());
        assert_eq!(err.to_string(), "runtime error: xorriso failed");
    }

    #[test]
    fn missing_tool_error_displays_message() {
        let err = EngineError::MissingTool("xorriso".to_string());
        assert_eq!(err.to_string(), "tooling missing: xorriso");
    }

    #[test]
    fn not_found_error_displays_message() {
        let err = EngineError::NotFound("/tmp/missing.iso".to_string());
        assert_eq!(err.to_string(), "not found: /tmp/missing.iso");
    }

    #[test]
    fn network_error_displays_message() {
        let err = EngineError::Network("status 404".to_string());
        assert_eq!(err.to_string(), "network error: status 404");
    }

    #[test]
    fn path_safety_error_displays_message() {
        let err = EngineError::PathSafety("path traversal detected".to_string());
        assert_eq!(
            err.to_string(),
            "filesystem safety violation: path traversal detected"
        );
    }

    #[test]
    fn policy_violation_error_displays_message() {
        let err = EngineError::PolicyViolation("license denied".to_string());
        assert_eq!(err.to_string(), "policy violation: license denied");
    }

    #[test]
    fn io_error_wraps_std_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = EngineError::Io(io_err);
        assert!(err.to_string().contains("io error:"));
    }
}
