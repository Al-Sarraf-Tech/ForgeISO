//! Ubuntu / cloud-init autoinstall YAML generation and merging.
//!
//! This module is split into:
//! * [`generate`] — produce a fresh autoinstall YAML document from an
//!   [`InjectConfig`](crate::config::InjectConfig).
//! * [`merge`] — merge an [`InjectConfig`] into an existing autoinstall YAML
//!   document, preserving fields the user already set unless explicitly
//!   overridden.
//!
//! The public surface (`generate_autoinstall_yaml`, `merge_autoinstall_yaml`)
//! is re-exported here so callers continue to import from
//! `crate::autoinstall::ubuntu::*` exactly as before.

mod generate;
mod merge;

pub use generate::generate_autoinstall_yaml;
pub use merge::merge_autoinstall_yaml;

#[cfg(test)]
mod tests;
