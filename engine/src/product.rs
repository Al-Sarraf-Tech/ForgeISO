#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidedWorkflowStep {
    Source,
    Configure,
    Build,
    OptionalChecks,
}

impl GuidedWorkflowStep {
    pub const ALL: [Self; 4] = [
        Self::Source,
        Self::Configure,
        Self::Build,
        Self::OptionalChecks,
    ];

    pub fn index(self) -> usize {
        match self {
            Self::Source => 0,
            Self::Configure => 1,
            Self::Build => 2,
            Self::OptionalChecks => 3,
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    pub fn one_based(self) -> i32 {
        self.index() as i32 + 1
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Choose ISO",
            Self::Configure => "Configure",
            Self::Build => "Build",
            Self::OptionalChecks => "Optional Checks",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Source => "Pick a source image",
            Self::Configure => "Required settings first",
            Self::Build => "Create the ISO",
            Self::OptionalChecks => "Extra validation only",
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            Self::Source => Some(Self::Configure),
            Self::Configure => Some(Self::Build),
            Self::Build => Some(Self::OptionalChecks),
            Self::OptionalChecks => None,
        }
    }

    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Source => None,
            Self::Configure => Some(Self::Source),
            Self::Build => Some(Self::Configure),
            Self::OptionalChecks => Some(Self::Build),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuidedWorkflowProgress {
    pub source_ready: bool,
    pub configure_done: bool,
    pub build_done: bool,
    pub verify_done: bool,
    pub iso9660_done: bool,
}

impl GuidedWorkflowProgress {
    pub fn step_complete(self, step: GuidedWorkflowStep) -> bool {
        match step {
            GuidedWorkflowStep::Source => self.source_ready,
            GuidedWorkflowStep::Configure => self.configure_done,
            GuidedWorkflowStep::Build => self.build_done,
            GuidedWorkflowStep::OptionalChecks => self.checks_run(),
        }
    }

    pub fn can_open_step(
        self,
        current_step: GuidedWorkflowStep,
        target_step: GuidedWorkflowStep,
    ) -> bool {
        match target_step {
            GuidedWorkflowStep::Source => true,
            GuidedWorkflowStep::Configure => {
                self.source_ready || current_step.index() >= GuidedWorkflowStep::Configure.index()
            }
            GuidedWorkflowStep::Build => {
                self.configure_done || current_step.index() >= GuidedWorkflowStep::Build.index()
            }
            GuidedWorkflowStep::OptionalChecks => {
                self.build_done
                    || current_step.index() >= GuidedWorkflowStep::OptionalChecks.index()
            }
        }
    }

    pub fn checks_run(self) -> bool {
        self.verify_done || self.iso9660_done
    }

    pub fn flow_complete(self) -> bool {
        self.build_done
    }

    pub fn optional_checks_summary(self) -> &'static str {
        if !self.build_done {
            "Build not finished"
        } else if self.checks_run() {
            "Optional checks complete"
        } else {
            "Optional checks skipped"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GuidedWorkflowProgress, GuidedWorkflowStep};

    #[test]
    fn build_completes_guided_flow_without_checks() {
        let progress = GuidedWorkflowProgress {
            source_ready: true,
            configure_done: true,
            build_done: true,
            verify_done: false,
            iso9660_done: false,
        };

        assert!(progress.flow_complete());
        assert_eq!(
            progress.optional_checks_summary(),
            "Optional checks skipped"
        );
        assert!(progress.can_open_step(
            GuidedWorkflowStep::Build,
            GuidedWorkflowStep::OptionalChecks
        ));
    }

    #[test]
    fn optional_checks_have_stable_product_labeling() {
        assert_eq!(GuidedWorkflowStep::Source.label(), "Choose ISO");
        assert_eq!(GuidedWorkflowStep::Source.subtitle(), "Pick a source image");
        assert_eq!(
            GuidedWorkflowStep::OptionalChecks.label(),
            "Optional Checks"
        );
        assert_eq!(
            GuidedWorkflowStep::OptionalChecks.subtitle(),
            "Extra validation only"
        );
    }

    #[test]
    fn step_completion_treats_optional_checks_as_separate_from_required_flow() {
        let progress = GuidedWorkflowProgress {
            source_ready: true,
            configure_done: true,
            build_done: true,
            verify_done: false,
            iso9660_done: false,
        };

        assert!(progress.step_complete(GuidedWorkflowStep::Build));
        assert!(!progress.step_complete(GuidedWorkflowStep::OptionalChecks));
    }

    #[test]
    fn one_based_index_round_trips() {
        let step = GuidedWorkflowStep::from_index(3).expect("step 4 exists");
        assert_eq!(step, GuidedWorkflowStep::OptionalChecks);
        assert_eq!(step.one_based(), 4);
        assert!(GuidedWorkflowStep::from_index(4).is_none());
    }
}
