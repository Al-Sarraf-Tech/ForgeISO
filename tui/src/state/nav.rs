//! Navigation primitives: wizard steps, focus, tabs, log/worker enums.
//!
//! Pure data — no behaviour beyond trivial helpers. Re-exported from
//! [`crate::state`] so existing imports keep working.

use forgeiso_engine::{
    BuildResult, GuidedWorkflowStep, Iso9660Compliance, IsoMetadata, VerifyResult,
};

#[allow(dead_code)]
pub(crate) enum WorkerMsg {
    InspectOk(Box<IsoMetadata>),
    InjectOk(Box<BuildResult>),
    EngineEvent(String, LogLevel),
    VerifyOk(Box<VerifyResult>),
    Iso9660Ok(Box<Iso9660Compliance>),
    OpError(String),
}

pub(crate) type WizardStep = GuidedWorkflowStep;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigTab {
    Identity,
    Network,
    Packages,
    Services,
    Advanced,
    Output,
}

impl ConfigTab {
    pub(crate) const ALL: [Self; 6] = [
        Self::Identity,
        Self::Network,
        Self::Packages,
        Self::Services,
        Self::Advanced,
        Self::Output,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Network => "Network",
            Self::Packages => "Packages",
            Self::Services => "Services",
            Self::Advanced => "Advanced",
            Self::Output => "Output",
        }
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0)
    }

    pub(crate) fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceFocus {
    PresetList,
    ManualInput,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
    Info,
    Warn,
    Error,
}

pub(crate) struct LogEntry {
    pub(crate) text: String,
    pub(crate) level: LogLevel,
}
