use std::path::Path;

use crate::error::EngineResult;

/// Patch grub.cfg and isolinux.cfg boot entries with additional kernel params.
pub(in crate::orchestrator) fn patch_boot_configs(
    extract_dir: &Path,
    kernel_append: &str,
) -> EngineResult<()> {
    // Patch grub.cfg — try both canonical kernel paths.
    // Ubuntu live/desktop/server ISOs use /casper/vmlinuz since 20.04.
    // Older ISOs (pre-20.04) and some remasters use /boot/vmlinuz.
    // Both use a literal tab between the `linux` keyword and the path.
    let grub_path = extract_dir.join("boot").join("grub").join("grub.cfg");
    if grub_path.exists() {
        let content = std::fs::read_to_string(&grub_path)?;
        // Replace whichever pattern is present; only one will match per ISO.
        let patched = content
            .replace(
                "linux\t/casper/vmlinuz",
                &format!("linux\t/casper/vmlinuz{}", kernel_append),
            )
            .replace(
                "linux\t/boot/vmlinuz",
                &format!("linux\t/boot/vmlinuz{}", kernel_append),
            );
        std::fs::write(&grub_path, patched)?;
    }

    // Patch isolinux.cfg — the append line contains the full kernel cmdline;
    // /vmlinuz matches as a substring of /casper/vmlinuz and /boot/vmlinuz.
    let isolinux_path = extract_dir.join("isolinux").join("isolinux.cfg");
    if isolinux_path.exists() {
        let content = std::fs::read_to_string(&isolinux_path)?;
        let patched = content.replace("/vmlinuz", &format!("/vmlinuz{}", kernel_append));
        std::fs::write(&isolinux_path, patched)?;
    }

    Ok(())
}

/// Patch an EFI `grub.cfg` by appending `kernel_append` to every kernel
/// command line (`linuxefi` / `linux` lines).
///
/// A global `.replace("quiet", ...)` was used previously but is incorrect:
/// - It corrupts comments and menu labels containing the search word.
/// - It silently skips the injection when the word is absent (e.g. Fedora
///   ISOs that don't include `quiet` in their EFI config).
///
/// This function appends unconditionally to every `linuxefi` / `linux` line,
/// which is safe: duplicate or additional kernel parameters are ignored by the
/// bootloader.
pub(in crate::orchestrator) fn patch_efi_grub_cfg(content: &str, kernel_append: &str) -> String {
    let mut patched_lines: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("linuxefi ") || trimmed.starts_with("linux ") {
            patched_lines.push(format!("{}{}", line.trim_end(), kernel_append));
        } else {
            patched_lines.push(line.to_string());
        }
    }
    patched_lines.join("\n") + "\n"
}
