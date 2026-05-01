//! Internal validators for [`super::InjectConfig`].
//!
//! The public entry point is [`super::InjectConfig::validate`], which calls
//! [`run`] in this module. Each helper is in a per-concern submodule and is
//! re-exported here as `pub(super) use` so the orchestrator can call them
//! by short names; their original behaviour is preserved exactly.

mod grub;
mod identity;
mod network;
mod output;
mod packages;
mod ssh;
mod storage;
mod system;

use crate::error::EngineResult;

use super::InjectConfig;

use grub::validate_grub;
use identity::validate_identity;
use network::{validate_network, validate_proxy};
use output::{validate_out_name, validate_output_label, validate_sha256};
use packages::{
    validate_apt_mirror, validate_apt_repos, validate_dnf, validate_packages, validate_pacman,
};
use ssh::{validate_containers, validate_ssh};
use storage::{
    validate_encryption, validate_mounts, validate_swap, validate_swap_size, validate_wallpaper,
};
use system::{
    validate_firewall, validate_services, validate_sysctl, validate_user_basics, validate_user_sudo,
};

/// Mirrors the exact validation order of the original monolithic
/// `InjectConfig::validate()`: identity -> user.basics -> services ->
/// firewall -> sysctl -> user.sudo -> apt_repos -> mounts -> apt_mirror ->
/// proxy -> network (static_ip/gateway/dns/ntp) -> ssh -> containers ->
/// swap -> output_label -> wallpaper -> grub -> out_name -> dnf -> pacman ->
/// sha256 -> swap_size -> encryption -> packages.
pub(super) fn run(cfg: &InjectConfig) -> EngineResult<()> {
    validate_identity(
        cfg.hostname.as_ref(),
        cfg.username.as_ref(),
        cfg.realname.as_ref(),
        cfg.timezone.as_ref(),
        cfg.locale.as_ref(),
        cfg.keyboard_layout.as_ref(),
    )?;

    validate_user_basics(&cfg.user)?;
    validate_services(&cfg.enable_services, &cfg.disable_services)?;
    validate_firewall(&cfg.firewall)?;
    validate_sysctl(&cfg.sysctl)?;
    validate_user_sudo(&cfg.user)?;

    validate_apt_repos(&cfg.apt_repos)?;
    validate_mounts(&cfg.mounts)?;
    validate_apt_mirror(cfg.apt_mirror.as_ref())?;

    validate_proxy(&cfg.proxy)?;
    validate_network(&cfg.network, cfg.static_ip.as_ref(), cfg.gateway.as_ref())?;

    validate_ssh(&cfg.ssh)?;
    validate_containers(&cfg.containers)?;

    validate_swap(cfg.swap.as_ref())?;
    validate_output_label(cfg.output_label.as_ref())?;
    validate_wallpaper(cfg.wallpaper.as_ref())?;
    validate_grub(&cfg.grub)?;
    validate_out_name(&cfg.out_name)?;

    validate_dnf(&cfg.dnf_repos, cfg.dnf_mirror.as_ref())?;
    validate_pacman(&cfg.pacman_repos, cfg.pacman_mirror.as_ref())?;

    validate_sha256(cfg.expected_sha256.as_ref())?;
    validate_swap_size(cfg.swap.as_ref())?;

    validate_encryption(
        cfg.encrypt,
        cfg.encrypt_passphrase.as_ref(),
        cfg.storage_layout.as_ref(),
    )?;

    validate_packages(&cfg.extra_packages)?;

    Ok(())
}
