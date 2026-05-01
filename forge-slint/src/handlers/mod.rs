//! Per-step callback handler modules.
//!
//! Each submodule exposes a `wire(&AppWindow)` function that registers
//! every Slint callback owned by that wizard step. `wire_all_handlers`
//! is the single entry point used by `main.rs` to install all callbacks
//! after window construction.
//!
//! The split mirrors the wizard layout:
//!
//! * [`source`]    — Step 1 (Source ISO selection)
//! * [`configure`] — Step 2 (Distro configuration)
//! * [`build`]     — Step 3 (Build / inject pipeline)
//! * [`check`]     — Step 4 (Verification + post-build actions)
//! * [`common`]    — Cross-step UI chrome (cancel, theme, doctor, log, step bar)

pub(crate) mod build;
pub(crate) mod check;
pub(crate) mod common;
pub(crate) mod configure;
pub(crate) mod source;

use crate::AppWindow;

/// Register every callback for the given window in a deterministic order.
///
/// Callbacks are registered before [`slint::ComponentHandle::run`] is
/// called. Registration order is not load-bearing; each `wire` function
/// only installs handlers and never fires them synchronously.
pub(crate) fn wire_all_handlers(win: &AppWindow) {
    common::wire(win);
    source::wire(win);
    configure::wire(win);
    build::wire(win);
    check::wire(win);
}
