mod late_commands;
mod ubuntu;

pub use late_commands::build_feature_late_commands;
pub use ubuntu::{generate_autoinstall_yaml, merge_autoinstall_yaml};

use sha_crypt::{sha512_simple, Sha512Params};

use crate::error::{EngineError, EngineResult};

/// Hash a plaintext password to SHA512-crypt format ($6$...)
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
pub fn hash_password(plaintext: &str) -> EngineResult<String> {
    let params = Sha512Params::new(10_000)
        .map_err(|e| EngineError::Runtime(format!("Failed to create SHA512 params: {e:?}")))?;
    sha512_simple(plaintext, &params)
        .map_err(|e| EngineError::Runtime(format!("Failed to hash password: {e:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password_format() {
        let hashed = hash_password("testpass").unwrap();
        assert!(hashed.starts_with("$6$"), "Hash should start with $6$");
    }

    #[test]
    fn hash_password_empty_string_produces_sha512_crypt() {
        // Empty password must hash without panicking — cloud-init may rely on it
        // for locked accounts that use SSH keys only.
        let h = hash_password("").expect("empty password must hash without error");
        assert!(h.starts_with("$6$"), "must be SHA-512-crypt format");
    }

    #[test]
    fn hash_password_unicode_input_produces_sha512_crypt() {
        // Non-ASCII passphrases must round-trip through the sha-crypt crate.
        let h = hash_password("p\u{00e1}ssw0rd\u{1f511}").expect("unicode password must hash");
        assert!(h.starts_with("$6$"), "hash format must be SHA-512-crypt");
    }

    #[test]
    fn hash_password_long_input_does_not_panic() {
        let long = "a".repeat(1024);
        let h = hash_password(&long).expect("long password must hash without error");
        assert!(h.starts_with("$6$"));
    }
}
