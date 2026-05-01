use serde::{Deserialize, Serialize};

/// Supported hypervisor targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hypervisor {
    Qemu,
    VirtualBox,
    Vmware,
    HyperV,
    Proxmox,
}

impl Hypervisor {
    /// Return a short lowercase identifier string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Hypervisor::Qemu => "qemu",
            Hypervisor::VirtualBox => "virtualbox",
            Hypervisor::Vmware => "vmware",
            Hypervisor::HyperV => "hyperv",
            Hypervisor::Proxmox => "proxmox",
        }
    }

    /// All hypervisor variants in a stable order.
    pub fn all() -> &'static [Hypervisor] {
        &[
            Hypervisor::Qemu,
            Hypervisor::VirtualBox,
            Hypervisor::Vmware,
            Hypervisor::HyperV,
            Hypervisor::Proxmox,
        ]
    }

    /// Parse from a lowercase string.  Returns `None` for unknown values.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "qemu" => Some(Hypervisor::Qemu),
            "virtualbox" | "vbox" => Some(Hypervisor::VirtualBox),
            "vmware" => Some(Hypervisor::Vmware),
            "hyperv" | "hyper-v" => Some(Hypervisor::HyperV),
            "proxmox" | "pve" => Some(Hypervisor::Proxmox),
            _ => None,
        }
    }
}

impl std::fmt::Display for Hypervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
