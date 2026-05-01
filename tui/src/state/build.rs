//! Step 3 (Build) and shared invalidation helpers on [`App`].
//!
//! Tracks build completion, clears downstream state when upstream inputs
//! change, and renders the human-readable summary shown on the Build step.

use super::App;

impl App {
    pub(crate) fn build_is_complete(&self) -> bool {
        self.progress.build_done
    }

    pub(crate) fn invalidate_build_and_checks(&mut self) {
        let artifact = self
            .build_artifact
            .as_ref()
            .map(|path| path.display().to_string());
        if artifact.as_deref() == Some(self.verify_source.as_str()) {
            self.verify_source.clear();
        }

        self.progress.configure_done = false;
        self.progress.build_done = false;
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.build_artifact = None;
        self.build_sha256 = None;
        self.verify_result = None;
        self.iso9660_result = None;
    }

    pub(crate) fn invalidate_checks_only(&mut self) {
        self.progress.verify_done = false;
        self.progress.iso9660_done = false;
        self.verify_result = None;
        self.iso9660_result = None;
    }

    pub(crate) fn summary_lines(&self) -> Vec<(String, String)> {
        let mut lines = Vec::new();
        let src = self.effective_source();
        if !src.is_empty() {
            lines.push(("Source".into(), src));
        }
        let add = |lines: &mut Vec<(String, String)>, label: &str, val: &str| {
            if !val.trim().is_empty() {
                lines.push((label.into(), val.trim().into()));
            }
        };
        add(&mut lines, "Hostname", &self.hostname);
        add(&mut lines, "Username", &self.username);
        if !self.password.is_empty() {
            lines.push(("Password".into(), "(set)".into()));
        }
        add(&mut lines, "Real Name", &self.realname);
        add(&mut lines, "Distro", &self.distro);
        if self.ssh_install_server {
            lines.push(("SSH Server".into(), "yes".into()));
        }
        add(&mut lines, "DNS", &self.dns_servers);
        add(&mut lines, "NTP", &self.ntp_servers);
        add(&mut lines, "Static IP", &self.static_ip);
        add(&mut lines, "Gateway", &self.gateway);
        add(&mut lines, "HTTP Proxy", &self.http_proxy);
        add(&mut lines, "HTTPS Proxy", &self.https_proxy);
        add(&mut lines, "Packages", &self.packages);
        add(&mut lines, "APT Repos", &self.apt_repos);
        add(&mut lines, "DNF Repos", &self.dnf_repos);
        if self.docker {
            lines.push(("Docker".into(), "yes".into()));
        }
        if self.podman {
            lines.push(("Podman".into(), "yes".into()));
        }
        if self.firewall_enabled {
            lines.push(("Firewall".into(), "enabled".into()));
        }
        add(&mut lines, "Timezone", &self.timezone);
        add(&mut lines, "Locale", &self.locale);
        add(&mut lines, "Storage", &self.storage_layout);
        if self.encrypt {
            lines.push(("Encrypt".into(), "yes".into()));
        }
        add(&mut lines, "Swap (MB)", &self.swap_size_mb);
        add(&mut lines, "Output Dir", &self.output_dir);
        add(&mut lines, "Output Name", &self.out_name);
        add(&mut lines, "Output Label", &self.output_label);
        lines
    }
}
