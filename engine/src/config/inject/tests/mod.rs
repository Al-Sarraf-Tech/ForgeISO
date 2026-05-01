//! Tests for [`super::InjectConfig::validate`].
//!
//! Split per concern out of the original `inject.rs` monolith. Each
//! submodule preserves its tests verbatim (assertion text, ordering,
//! fixtures); `super::super::*` brings the `InjectConfig` parent module
//! items into scope for every submodule.

mod identity;
mod network;
mod output;
mod packages;
mod storage;
mod system;
