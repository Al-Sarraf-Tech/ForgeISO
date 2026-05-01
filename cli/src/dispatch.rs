//! Dispatch wired-up `Commands` variants to their handler in `crate::handlers`.

use forgeiso_engine::ForgeIsoEngine;

use crate::cli::Commands;
use crate::handlers;

#[allow(clippy::too_many_lines)]
pub(crate) async fn run(engine: &ForgeIsoEngine, command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Doctor { json } => {
            handlers::doctor::handle(engine, json).await?;
        }
        Commands::Inspect { source, json } => {
            handlers::inspect::handle(engine, source, json).await?;
        }
        Commands::Build {
            source,
            preset,
            project,
            out,
            name,
            overlay,
            volume_label,
            profile,
            expected_sha256,
            json,
        } => {
            handlers::build::handle(
                engine,
                source,
                preset,
                project,
                out,
                name,
                overlay,
                volume_label,
                profile,
                expected_sha256,
                json,
            )
            .await?;
        }
        Commands::Scan {
            artifact,
            policy,
            json,
        } => {
            handlers::scan::handle(engine, artifact, policy, json).await?;
        }
        Commands::Test {
            iso,
            bios,
            uefi,
            json,
        } => {
            handlers::test_iso::handle(engine, iso, bios, uefi, json).await?;
        }
        Commands::Report { build, format } => {
            handlers::report::handle(engine, build, format).await?;
        }
        Commands::Verify {
            source,
            sums_url,
            json,
        } => {
            handlers::verify::handle(engine, source, sums_url, json).await?;
        }
        Commands::Inject {
            source,
            preset,
            autoinstall,
            out,
            name,
            volume_label,
            hostname,
            username,
            password,
            password_file,
            password_stdin,
            realname,
            ssh_key,
            ssh_key_file,
            ssh_password_auth,
            no_ssh_password_auth,
            ssh_install_server,
            no_ssh_install_server,
            dns,
            ntp_server,
            timezone,
            locale,
            keyboard_layout,
            storage_layout,
            apt_mirror,
            package,
            wallpaper,
            late_command,
            no_user_interaction,
            group,
            shell,
            sudo_nopasswd,
            sudo_command,
            firewall,
            firewall_policy,
            allow_port,
            deny_port,
            static_ip,
            gateway,
            http_proxy,
            https_proxy,
            no_proxy,
            enable_service,
            disable_service,
            sysctl,
            swap_size,
            swap_file,
            swappiness,
            apt_repo,
            dnf_repo,
            dnf_mirror,
            pacman_repo,
            pacman_mirror,
            docker,
            podman,
            docker_user,
            grub_timeout,
            grub_cmdline,
            grub_default,
            encrypt,
            encrypt_passphrase,
            encrypt_passphrase_file,
            mount,
            run_command,
            distro,
            expected_sha256,
            json,
        } => {
            handlers::inject::handle(
                engine,
                source,
                preset,
                autoinstall,
                out,
                name,
                volume_label,
                hostname,
                username,
                password,
                password_file,
                password_stdin,
                realname,
                ssh_key,
                ssh_key_file,
                ssh_password_auth,
                no_ssh_password_auth,
                ssh_install_server,
                no_ssh_install_server,
                dns,
                ntp_server,
                timezone,
                locale,
                keyboard_layout,
                storage_layout,
                apt_mirror,
                package,
                wallpaper,
                late_command,
                run_command,
                no_user_interaction,
                group,
                shell,
                sudo_nopasswd,
                sudo_command,
                firewall,
                firewall_policy,
                allow_port,
                deny_port,
                static_ip,
                gateway,
                http_proxy,
                https_proxy,
                no_proxy,
                enable_service,
                disable_service,
                sysctl,
                swap_size,
                swap_file,
                swappiness,
                apt_repo,
                dnf_repo,
                dnf_mirror,
                pacman_repo,
                pacman_mirror,
                docker,
                podman,
                docker_user,
                grub_timeout,
                grub_cmdline,
                grub_default,
                encrypt,
                encrypt_passphrase,
                encrypt_passphrase_file,
                mount,
                distro,
                expected_sha256,
                json,
            )
            .await?;
        }
        Commands::Diff { base, target, json } => {
            handlers::diff::handle(engine, base, target, json).await?;
        }
        Commands::Sources { command } => {
            handlers::sources::handle(command).await?;
        }
        Commands::Vm { command } => {
            handlers::vm::handle(command).await?;
        }
    }
    Ok(())
}
