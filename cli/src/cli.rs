//! Clap definitions for the `forgeiso` CLI.
//!
//! Extracted from `main.rs` so the binary entry point can stay small and the
//! command schema lives in one place.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub(crate) const CLI_AFTER_HELP: &str = "\
GUIDED INTERFACES:
    forgeiso-desktop  Desktop wizard for normal users
    forgeiso-tui      Guided terminal workflow for operators

ADVANCED CLI:
    forgeiso is the explicit interface for scripting, CI, and power-user workflows.
    Use `forgeiso inject`, `build`, `scan`, `test`, and `report` directly when you
    want stable flags, machine-readable output, or full control over the pipeline.
";

#[derive(Debug, Parser)]
#[command(
    name = "forgeiso",
    version,
    about = "Advanced CLI for local Linux ISO automation",
    arg_required_else_help = true,
    after_help = CLI_AFTER_HELP
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Commands {
    /// Check host tooling prerequisites (xorriso, qemu, trivy, etc.)
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Read ISO metadata (distro, release, arch, SHA-256) from a local file or URL
    Inspect {
        #[arg(long)]
        source: String,
        #[arg(long)]
        json: bool,
    },
    /// Build a customised ISO artifact from a source image and optional overlay
    Build {
        #[arg(long, conflicts_with = "preset", help_heading = "Source")]
        source: Option<String>,
        /// Use a built-in source preset instead of --source.
        /// Run 'forgeiso sources list' to see available presets.
        #[arg(long, conflicts_with = "source", help_heading = "Source")]
        preset: Option<String>,
        /// Load a build definition from a project file instead of individual flags.
        #[arg(long, help_heading = "Source")]
        project: Option<PathBuf>,
        /// Directory to write the output ISO into
        #[arg(long, help_heading = "Output")]
        out: PathBuf,
        /// Output ISO filename (e.g., my-server.iso)
        #[arg(long, help_heading = "Output")]
        name: Option<String>,
        /// Directory of files to overlay into the ISO root filesystem.
        #[arg(long, help_heading = "Output")]
        overlay: Option<PathBuf>,
        #[arg(long, help_heading = "Output")]
        volume_label: Option<String>,
        /// Build profile: minimal or desktop.
        #[arg(
            long,
            help_heading = "Output",
            value_name = "PROFILE",
            value_parser = ["minimal", "desktop"]
        )]
        profile: Option<String>,
        /// Expected SHA-256 hex digest of the source ISO; operation aborts if it does not match.
        #[arg(long, help_heading = "Output")]
        expected_sha256: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run security scans on a built ISO artifact (trivy, syft, grype, oscap)
    Scan {
        /// Built ISO artifact to scan.
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Boot-test an ISO in QEMU (BIOS and/or UEFI) as a smoke test
    Test {
        #[arg(long)]
        iso: PathBuf,
        #[arg(long)]
        bios: bool,
        #[arg(long)]
        uefi: bool,
        #[arg(long)]
        json: bool,
    },
    /// Generate an HTML or JSON report from a build artifact directory
    Report {
        #[arg(long)]
        build: PathBuf,
        /// Report format: html or json.
        #[arg(long, value_name = "FORMAT", value_parser = ["html", "json"])]
        format: String,
    },
    /// Verify ISO integrity against upstream SHA-256 checksums
    Verify {
        #[arg(long)]
        source: String,
        #[arg(long)]
        sums_url: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Inject a distro-appropriate unattended install configuration into an ISO
    #[command(after_long_help = "\
EXAMPLES:
    # Minimal Ubuntu server ISO
    forgeiso inject --source ubuntu-24.04.iso --out /tmp --hostname myserver --username admin --password secret

    # Fedora with Docker and firewall
    forgeiso inject --preset fedora-server --out /tmp --hostname web01 --username ops --password secret --docker --firewall --allow-port 22 --allow-port 443

    # From preset with SSH keys
    forgeiso inject --preset ubuntu-server-lts --out /tmp --hostname prod01 --username deploy --ssh-key \"ssh-ed25519 AAAA... user@host\" --sudo-nopasswd
")]
    Inject {
        #[arg(long, conflicts_with = "preset", help_heading = "Source")]
        source: Option<String>,
        /// Use a built-in source preset instead of --source.
        /// Run 'forgeiso sources list' to see available presets.
        #[arg(long, conflicts_with = "source", help_heading = "Source")]
        preset: Option<String>,
        /// Merge CLI flags into an existing Ubuntu autoinstall YAML.
        #[arg(long, help_heading = "Source")]
        autoinstall: Option<PathBuf>,
        /// Installer path override: ubuntu, fedora, mint, or arch.
        #[arg(
            long,
            value_name = "DISTRO",
            help_heading = "Source",
            value_parser = ["ubuntu", "fedora", "mint", "arch"]
        )]
        distro: Option<String>,
        #[arg(long, help_heading = "Source")]
        json: bool,

        // Identity
        #[arg(long, help_heading = "Identity")]
        hostname: Option<String>,
        #[arg(long, help_heading = "Identity")]
        username: Option<String>,
        #[arg(long, help_heading = "Identity")]
        password: Option<String>,
        #[arg(long, help_heading = "Identity")]
        password_file: Option<PathBuf>,
        #[arg(long, help_heading = "Identity")]
        password_stdin: bool,
        #[arg(long, help_heading = "Identity")]
        realname: Option<String>,

        // SSH
        #[arg(long, action = clap::ArgAction::Append, help_heading = "SSH")]
        ssh_key: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "SSH")]
        ssh_key_file: Vec<PathBuf>,
        #[arg(long, help_heading = "SSH")]
        ssh_password_auth: bool,
        #[arg(long, help_heading = "SSH")]
        no_ssh_password_auth: bool,
        #[arg(long, help_heading = "SSH")]
        ssh_install_server: bool,
        #[arg(long, help_heading = "SSH")]
        no_ssh_install_server: bool,

        // Network
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Network")]
        dns: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Network")]
        ntp_server: Vec<String>,
        #[arg(long, help_heading = "Network")]
        static_ip: Option<String>,
        #[arg(long, help_heading = "Network")]
        gateway: Option<String>,
        #[arg(long, help_heading = "Network")]
        http_proxy: Option<String>,
        #[arg(long, help_heading = "Network")]
        https_proxy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Network")]
        no_proxy: Vec<String>,

        // System
        #[arg(long, help_heading = "System")]
        timezone: Option<String>,
        #[arg(long, help_heading = "System")]
        locale: Option<String>,
        #[arg(long, help_heading = "System")]
        keyboard_layout: Option<String>,
        #[arg(long, help_heading = "System")]
        storage_layout: Option<String>,
        /// Override the Ubuntu/Debian package mirror used during install.
        #[arg(long, help_heading = "System")]
        apt_mirror: Option<String>,

        // Packages & Repos
        /// Extra package to install. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        package: Vec<String>,
        /// Additional APT repository entry. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        apt_repo: Vec<String>,
        /// Additional DNF repository stanza or URL. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        dnf_repo: Vec<String>,
        /// Override the primary Fedora/RHEL DNF mirror base URL
        #[arg(long, help_heading = "Packages & Repos")]
        dnf_mirror: Option<String>,
        /// Additional pacman repository mirror line. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Packages & Repos")]
        pacman_repo: Vec<String>,
        /// Override the primary Arch Linux pacman mirror URL
        #[arg(long, help_heading = "Packages & Repos")]
        pacman_mirror: Option<String>,

        // User & Access
        #[arg(long, action = clap::ArgAction::Append, help_heading = "User & Access")]
        group: Vec<String>,
        #[arg(long, help_heading = "User & Access")]
        shell: Option<String>,
        #[arg(long, help_heading = "User & Access")]
        sudo_nopasswd: bool,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "User & Access")]
        sudo_command: Vec<String>,

        // Firewall
        #[arg(long, help_heading = "Firewall")]
        firewall: bool,
        /// Firewall default policy, e.g. deny or reject.
        #[arg(long, help_heading = "Firewall")]
        firewall_policy: Option<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Firewall")]
        allow_port: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Firewall")]
        deny_port: Vec<String>,

        // Services
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Services")]
        enable_service: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Services")]
        disable_service: Vec<String>,

        // Containers
        #[arg(long, help_heading = "Containers")]
        docker: bool,
        #[arg(long, help_heading = "Containers")]
        podman: bool,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Containers")]
        docker_user: Vec<String>,

        // Storage & Encryption
        #[arg(long, help_heading = "Storage & Encryption")]
        swap_size: Option<u32>,
        #[arg(long, help_heading = "Storage & Encryption")]
        swap_file: Option<String>,
        #[arg(long, help_heading = "Storage & Encryption")]
        swappiness: Option<u8>,
        #[arg(long, help_heading = "Storage & Encryption")]
        encrypt: bool,
        #[arg(long, help_heading = "Storage & Encryption")]
        encrypt_passphrase: Option<String>,
        #[arg(long, help_heading = "Storage & Encryption")]
        encrypt_passphrase_file: Option<PathBuf>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Storage & Encryption")]
        mount: Vec<String>,

        // Boot
        #[arg(long, help_heading = "Boot")]
        grub_timeout: Option<u32>,
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Boot")]
        grub_cmdline: Vec<String>,
        #[arg(long, help_heading = "Boot")]
        grub_default: Option<String>,

        // Advanced
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Advanced")]
        sysctl: Vec<String>,
        /// Shell command to run AFTER install (in chroot). Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Advanced")]
        late_command: Vec<String>,
        /// Shell command to run BEFORE install. Repeatable.
        #[arg(long, action = clap::ArgAction::Append, help_heading = "Advanced")]
        run_command: Vec<String>,
        /// Finish without prompting during the target install when supported.
        #[arg(long, help_heading = "Advanced")]
        no_user_interaction: bool,
        /// Wallpaper asset to copy into supported desktop installers.
        #[arg(long, help_heading = "Advanced")]
        wallpaper: Option<PathBuf>,

        // Output
        /// Directory to write the output ISO into
        #[arg(long, help_heading = "Output")]
        out: PathBuf,
        /// Output ISO filename (e.g., my-server.iso)
        #[arg(long, help_heading = "Output")]
        name: Option<String>,
        #[arg(long, help_heading = "Output")]
        volume_label: Option<String>,
        /// Expected SHA-256 hex digest of the source ISO; operation aborts if it does not match.
        #[arg(long, help_heading = "Output")]
        expected_sha256: Option<String>,
    },
    /// Show which files differ between two ISO images
    Diff {
        #[arg(long)]
        base: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Work with built-in ISO source presets
    Sources {
        #[command(subcommand)]
        command: SourcesCmd,
    },
    /// Generate VM hypervisor launch commands for a local ISO
    Vm {
        #[command(subcommand)]
        command: VmCmd,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SourcesCmd {
    /// List all built-in ISO source presets
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show details for a specific preset
    Show {
        /// Preset name, e.g. ubuntu-server-lts
        preset: String,
        #[arg(long)]
        json: bool,
    },
    /// Resolve the download URL for a preset
    Resolve {
        /// Preset name, e.g. ubuntu-server-lts
        preset: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum VmCmd {
    /// Emit hypervisor launch commands for testing an ISO in a VM
    Emit {
        /// Path to the ISO to boot
        #[arg(long, value_name = "PATH")]
        iso: PathBuf,
        /// Hypervisor: qemu (default), virtualbox, vmware, hyperv, proxmox
        #[arg(long, default_value = "qemu")]
        hypervisor: String,
        /// Firmware: bios (default) or uefi
        #[arg(long, default_value = "bios")]
        firmware: String,
        /// RAM in MiB (default: 2048)
        #[arg(long, default_value_t = 2048)]
        ram: u32,
        /// vCPUs (default: 2)
        #[arg(long, default_value_t = 2)]
        cpus: u8,
        /// Disk size in GiB for QEMU disk image creation (default: 20)
        #[arg(long, default_value_t = 20)]
        disk: u32,
        /// VM name (defaults to ISO stem)
        #[arg(long)]
        name: Option<String>,
        /// Emit output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::{CommandFactory, Subcommand};

    fn render_help(mut command: clap::Command) -> String {
        let mut buffer = Vec::new();
        command
            .write_long_help(&mut buffer)
            .expect("help should render");
        String::from_utf8(buffer).expect("help should be utf-8")
    }

    #[test]
    fn root_help_frames_guided_and_advanced_interfaces() {
        let help = render_help(Cli::command());
        assert!(help.contains("forgeiso-desktop"));
        assert!(help.contains("forgeiso-tui"));
        assert!(help.contains("Advanced CLI for local Linux ISO automation"));
    }

    #[test]
    fn inject_help_mentions_archinstall_and_valid_distros() {
        let inject = Commands::augment_subcommands(clap::Command::new("forgeiso"))
            .find_subcommand("inject")
            .expect("inject subcommand should exist")
            .clone();
        let help = render_help(inject);
        assert!(help.contains("distro-appropriate unattended install configuration"));
        assert!(help.contains("ubuntu"));
        assert!(help.contains("fedora"));
        assert!(help.contains("mint"));
        assert!(help.contains("arch"));
    }

    #[test]
    fn scan_help_uses_artifact_flag() {
        let scan = Commands::augment_subcommands(clap::Command::new("forgeiso"))
            .find_subcommand("scan")
            .expect("scan subcommand should exist")
            .clone();
        let help = render_help(scan);
        assert!(help.contains("--artifact"));
        assert!(!help.contains("--source"));
    }
}
