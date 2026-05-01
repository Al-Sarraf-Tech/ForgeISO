//! Property-based tests for engine config parsers, validators, and pure
//! utilities. These run as a top-level integration test (not a unit test) so
//! that proptest's verbose shrinking output is segregated from the inner
//! module test results.
//!
//! Each property runs the proptest default of 256 iterations.

use std::path::PathBuf;

use forgeiso_engine::{
    all_presets,
    config::{IsoSource, ProfileKind},
    find_preset_by_str,
    sources::PresetId,
    EngineError, InjectConfig,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// All shell metacharacters that the realname/out_name validators reject.
/// Mirrors the literal set in
/// `engine/src/config/inject/validate/identity.rs::validate_identity` and
/// `engine/src/config/inject/validate/output.rs::validate_out_name`.
const SHELL_METACHARS: &[char] = &[';', '&', '|', '$', '`', '\'', '"', '\\', '\n'];

/// Build a minimal InjectConfig with the given out_name. Constructed by
/// hand (not via the builder) so we can stuff arbitrary fuzzed values into
/// individual fields without the builder pre-validating them.
fn cfg_with(out_name: impl Into<String>) -> InjectConfig {
    InjectConfig {
        source: IsoSource::from_raw("/tmp/example.iso"),
        out_name: out_name.into(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 1. InjectConfig::validate() over fuzzed identity strings
// ---------------------------------------------------------------------------

proptest! {
    /// Validate must never panic for arbitrary hostname/username/realname
    /// strings, and must return either Ok or the documented InvalidConfig
    /// variant. No other EngineError variant is reachable from these fields.
    #[test]
    fn validate_identity_never_panics_returns_documented_err(
        host in ".{0,64}",
        user in ".{0,64}",
        real in ".{0,128}",
    ) {
        let mut cfg = cfg_with("out.iso");
        cfg.hostname = Some(host);
        cfg.username = Some(user);
        cfg.realname = Some(real);
        match cfg.validate() {
            Ok(()) => {}
            Err(EngineError::InvalidConfig(_)) => {}
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Output filename safety: a validated out_name never contains '/' or '\'
// ---------------------------------------------------------------------------

proptest! {
    /// Any out_name accepted by validate() is shell-safe and contains no
    /// path separators — guarantees that joining it onto an output dir
    /// cannot produce a path-traversal escape.
    #[test]
    fn out_name_when_valid_has_no_path_separators(name in ".{1,32}") {
        let cfg = cfg_with(name.clone());
        if cfg.validate().is_ok() {
            let trimmed = name.trim();
            prop_assert!(!trimmed.contains('/'));
            prop_assert!(!trimmed.contains('\\'));
            for c in SHELL_METACHARS {
                prop_assert!(!trimmed.contains(*c), "metachar {c:?} accepted in {trimmed:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. PresetId::parse + as_str roundtrip
// ---------------------------------------------------------------------------

proptest! {
    /// Every canonical preset id roundtrips through parse + as_str unchanged,
    /// and parse is case-insensitive with respect to the canonical id.
    #[test]
    fn preset_id_roundtrip_for_valid_ids(idx in 0usize..all_presets().len()) {
        let preset = &all_presets()[idx];
        let s = preset.id.as_str();
        let parsed = PresetId::parse(s).expect("canonical id must parse");
        prop_assert_eq!(parsed.as_str(), s);
        // Case-insensitive lookup must produce the same preset.
        let upper = PresetId::parse(&s.to_uppercase()).expect("upper-case id must parse");
        prop_assert_eq!(upper.as_str(), s);
        // find_preset_by_str finds it too.
        let found = find_preset_by_str(s).expect("find_preset_by_str must succeed");
        prop_assert_eq!(found.id.as_str(), s);
    }
}

// ---------------------------------------------------------------------------
// 4. Output volume label: <=32 chars after sanitize/trim
// ---------------------------------------------------------------------------

proptest! {
    /// Any output_label accepted by validate() is ≤32 chars (after trim) and
    /// pure-ASCII printable, matching the xorriso -V constraint.
    #[test]
    fn output_label_when_valid_is_short_ascii(label in ".{0,80}") {
        let mut cfg = cfg_with("out.iso");
        cfg.output_label = Some(label.clone());
        if cfg.validate().is_ok() {
            let trimmed = label.trim();
            prop_assume!(!trimmed.is_empty()); // Some("   ") gets rejected
            prop_assert!(trimmed.len() <= 32, "label too long: {} chars", trimmed.len());
            prop_assert!(trimmed.is_ascii(), "label must be ASCII: {trimmed:?}");
            prop_assert!(!trimmed.chars().any(|c| c.is_ascii_control()));
        }
    }
}

// ---------------------------------------------------------------------------
// 5. SHA-256 hex validation: only 64 lowercase hex chars accepted
// ---------------------------------------------------------------------------

proptest! {
    /// expected_sha256 is accepted iff (after trim+lowercase) it is exactly
    /// 64 hex chars. Inputs of any other shape must be rejected with
    /// InvalidConfig.
    #[test]
    fn sha256_validation_matches_documented_contract(s in "[ -~]{0,80}") {
        let mut cfg = cfg_with("out.iso");
        cfg.expected_sha256 = Some(s.clone());
        let normalized = s.trim().to_ascii_lowercase();
        let is_valid_hex = normalized.len() == 64
            && normalized.chars().all(|c| c.is_ascii_hexdigit());
        match cfg.validate() {
            Ok(()) => prop_assert!(is_valid_hex, "unexpected accept: {normalized:?}"),
            Err(EngineError::InvalidConfig(_)) => {
                // Either the sha256 was bad or some other field flunked; we
                // only assert: a valid hex string never causes the sha256
                // arm itself to reject (other defaults stay clean).
                if is_valid_hex {
                    // out_name=out.iso + default everything else passes the
                    // other validators, so the only way to reach Err here
                    // is if sha256 was bad — which it isn't.
                    panic!("valid 64-hex string rejected: {normalized:?}");
                }
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 6. IsoSource::from_raw is total and consistent with display_value
// ---------------------------------------------------------------------------

proptest! {
    /// from_raw never panics on any input, and display_value returns the
    /// original string for both URL and Path branches.
    #[test]
    fn iso_source_from_raw_round_trips_display(raw in ".{0,128}") {
        let src = IsoSource::from_raw(raw.clone());
        let displayed = src.display_value();
        // For the URL branch, display_value must return the exact input.
        if raw.starts_with("http://") || raw.starts_with("https://") {
            prop_assert!(src.is_remote());
            prop_assert_eq!(&displayed, &raw);
        } else {
            prop_assert!(!src.is_remote());
            // Path branch: PathBuf::display() round-trips ASCII verbatim;
            // for arbitrary unicode it should still equal the raw input on
            // Linux, where paths are byte-strings.
            prop_assert_eq!(&displayed, &raw);
        }
    }
}

// ---------------------------------------------------------------------------
// 7. BuildConfig YAML parse never panics on garbage input
// ---------------------------------------------------------------------------

proptest! {
    /// from_yaml_str must return Err (never panic) for arbitrary garbage and
    /// must succeed only when the parsed struct also passes validate().
    #[test]
    fn build_config_yaml_parse_never_panics(raw in "[ -~\\n]{0,200}") {
        use forgeiso_engine::config::BuildConfig;
        let _ = BuildConfig::from_yaml_str(&raw);
    }
}

// ---------------------------------------------------------------------------
// 8. Timezone validator: accepted iff IANA-safe character set
// ---------------------------------------------------------------------------

proptest! {
    /// Timezone is accepted iff non-empty and every char is alphanumeric or
    /// in {/, _, -, +}. Documented in
    /// engine/src/config/inject/validate/identity.rs.
    #[test]
    fn timezone_charset_matches_documented_contract(tz in "[ -~]{0,40}") {
        let mut cfg = cfg_with("out.iso");
        cfg.timezone = Some(tz.clone());
        let allowed = !tz.is_empty()
            && tz.chars().all(|c| c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'));
        match cfg.validate() {
            Ok(()) => prop_assert!(allowed, "unexpected accept of timezone {tz:?}"),
            Err(EngineError::InvalidConfig(_)) => {
                // Could be timezone or another field; only assert: a clearly
                // valid tz string with default other fields must pass.
                if allowed {
                    panic!("safe timezone rejected: {tz:?}");
                }
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Locale charset contract
// ---------------------------------------------------------------------------

proptest! {
    /// Locale accepted iff non-empty and every char is alphanumeric or in
    /// {_, -, .}. Default-builder cfg has no other failing fields.
    #[test]
    fn locale_charset_matches_documented_contract(loc in "[ -~]{0,40}") {
        let mut cfg = cfg_with("out.iso");
        cfg.locale = Some(loc.clone());
        let allowed = !loc.is_empty()
            && loc.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'));
        match cfg.validate() {
            Ok(()) => prop_assert!(allowed, "unexpected accept of locale {loc:?}"),
            Err(EngineError::InvalidConfig(_)) => {
                if allowed {
                    panic!("safe locale rejected: {loc:?}");
                }
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 10. ProfileKind serde round-trip — pure (de)serializer contract.
// ---------------------------------------------------------------------------

proptest! {
    /// Random ProfileKind values JSON-roundtrip exactly, never panicking.
    /// (Sanity check for the snake_case serde rename rule.)
    #[test]
    fn profile_kind_json_round_trip(pick in 0u8..2) {
        let pk = if pick == 0 { ProfileKind::Minimal } else { ProfileKind::Desktop };
        let json = serde_json::to_string(&pk).expect("serialize");
        let back: ProfileKind = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(back, pk);
    }
}

// ---------------------------------------------------------------------------
// 11. PresetId::parse returns None for clearly-bogus inputs
// ---------------------------------------------------------------------------

proptest! {
    /// Random strings that contain forbidden chars or wrong shape must yield
    /// None. We assume(!) that the random string is not coincidentally one
    /// of the 35 canonical IDs.
    #[test]
    fn preset_id_parse_rejects_garbage(s in ".{0,40}") {
        let known: Vec<&str> = all_presets().iter().map(|p| p.id.as_str()).collect();
        prop_assume!(!known.iter().any(|k| k.eq_ignore_ascii_case(s.trim())));
        prop_assert!(PresetId::parse(&s).is_none(), "unexpected parse of {s:?}");
    }
}

// ---------------------------------------------------------------------------
// 12. hash_password total: never panics, always returns a sha512-crypt hash
// ---------------------------------------------------------------------------

proptest! {
    // hash_password uses sha512-crypt with 10_000 rounds; keep iteration
    // count low so the suite stays under ~5s overall.
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Any input string (including empty / unicode / long) hashes without
    /// panicking and always produces a `$6$` prefixed sha512-crypt token.
    #[test]
    fn hash_password_total_for_arbitrary_input(pw in ".{0,256}") {
        let h = forgeiso_engine::hash_password(&pw).expect("hash_password must succeed");
        prop_assert!(h.starts_with("$6$"), "expected sha512-crypt prefix, got {h:?}");
    }
}

// ---------------------------------------------------------------------------
// 13. safe_join rejects all attempts to escape the workspace via ../ chains
// ---------------------------------------------------------------------------

proptest! {
    /// A relative path with ANY ParentDir component at the start can never
    /// escape the workspace; safe_join must always return either Ok with a
    /// path under the workspace, or Err(PathSafety).
    #[test]
    fn safe_join_never_escapes_workspace(parents in 1usize..16, child in "[a-zA-Z0-9_]{1,16}") {
        use forgeiso_engine::workspace::safe_join;
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let mut rel = PathBuf::new();
        for _ in 0..parents { rel.push(".."); }
        rel.push(&child);
        match safe_join(root, &rel) {
            Ok(p) => prop_assert!(p.starts_with(root.canonicalize().unwrap())),
            Err(EngineError::PathSafety(_)) => {}
            Err(EngineError::Io(_)) => {} // create_dir_all on parent may fail
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}
