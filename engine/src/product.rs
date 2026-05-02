//! Guided four-step workflow model that the GUI and TUI render.
//!
//! The product surface intentionally hides engine concepts the user
//! does not need to know about (workspaces, circuit breakers, scan
//! reports, etc.) and presents a simple progression:
//!
//! 1. Source — pick / download the upstream ISO.
//! 2. Configure — fill in identity, network, packages, services.
//! 3. Build — run the engine end-to-end.
//! 4. OptionalChecks — verify, scan, boot-test the result.
//!
//! The CLI bypasses the guided model entirely; it accepts
//! [`crate::BuildConfig`] directly. The GUI uses
//! [`GuidedWorkflowProgress`] to render the step rail and gate the
//! "Continue" button.

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

    #[test]
    fn next_chains_through_all_steps_then_terminates() {
        assert_eq!(
            GuidedWorkflowStep::Source.next(),
            Some(GuidedWorkflowStep::Configure)
        );
        assert_eq!(
            GuidedWorkflowStep::Configure.next(),
            Some(GuidedWorkflowStep::Build)
        );
        assert_eq!(
            GuidedWorkflowStep::Build.next(),
            Some(GuidedWorkflowStep::OptionalChecks)
        );
        assert_eq!(GuidedWorkflowStep::OptionalChecks.next(), None);
    }

    #[test]
    fn prev_chains_back_to_source_then_terminates() {
        assert_eq!(GuidedWorkflowStep::Source.prev(), None);
        assert_eq!(
            GuidedWorkflowStep::Configure.prev(),
            Some(GuidedWorkflowStep::Source)
        );
        assert_eq!(
            GuidedWorkflowStep::Build.prev(),
            Some(GuidedWorkflowStep::Configure)
        );
        assert_eq!(
            GuidedWorkflowStep::OptionalChecks.prev(),
            Some(GuidedWorkflowStep::Build)
        );
    }

    #[test]
    fn step_index_round_trips() {
        for s in GuidedWorkflowStep::ALL {
            let idx = s.index();
            assert_eq!(GuidedWorkflowStep::from_index(idx), Some(s));
        }
    }

    #[test]
    fn label_and_subtitle_distinct_for_each_step() {
        assert_eq!(GuidedWorkflowStep::Configure.label(), "Configure");
        assert_eq!(
            GuidedWorkflowStep::Configure.subtitle(),
            "Required settings first"
        );
        assert_eq!(GuidedWorkflowStep::Build.label(), "Build");
        assert_eq!(GuidedWorkflowStep::Build.subtitle(), "Create the ISO");
    }

    #[test]
    fn can_open_step_blocks_skipping_required_predecessors() {
        // No source ready, on Source step -> can NOT open Configure
        let blocked = GuidedWorkflowProgress::default();
        assert!(blocked.can_open_step(GuidedWorkflowStep::Source, GuidedWorkflowStep::Source));
        assert!(!blocked.can_open_step(GuidedWorkflowStep::Source, GuidedWorkflowStep::Configure));
        assert!(!blocked.can_open_step(GuidedWorkflowStep::Source, GuidedWorkflowStep::Build));
        assert!(!blocked.can_open_step(
            GuidedWorkflowStep::Source,
            GuidedWorkflowStep::OptionalChecks
        ));
    }

    #[test]
    fn can_open_step_allows_revisiting_current_or_earlier_step() {
        let progress = GuidedWorkflowProgress {
            source_ready: false,
            configure_done: false,
            build_done: false,
            verify_done: false,
            iso9660_done: false,
        };
        // Already on Configure -> may revisit Configure even without source_ready
        assert!(
            progress.can_open_step(GuidedWorkflowStep::Configure, GuidedWorkflowStep::Configure)
        );
        // Source is always openable
        assert!(progress.can_open_step(
            GuidedWorkflowStep::OptionalChecks,
            GuidedWorkflowStep::Source
        ));
    }

    #[test]
    fn checks_run_returns_true_when_either_check_done() {
        let mut p = GuidedWorkflowProgress::default();
        assert!(!p.checks_run());
        p.verify_done = true;
        assert!(p.checks_run());
        let q = GuidedWorkflowProgress {
            iso9660_done: true,
            ..Default::default()
        };
        assert!(q.checks_run());
    }

    #[test]
    fn optional_checks_summary_branches_correctly() {
        let none_done = GuidedWorkflowProgress::default();
        assert_eq!(none_done.optional_checks_summary(), "Build not finished");

        let built_only = GuidedWorkflowProgress {
            build_done: true,
            ..Default::default()
        };
        assert_eq!(
            built_only.optional_checks_summary(),
            "Optional checks skipped"
        );

        let built_and_checked = GuidedWorkflowProgress {
            build_done: true,
            verify_done: true,
            ..Default::default()
        };
        assert_eq!(
            built_and_checked.optional_checks_summary(),
            "Optional checks complete"
        );
    }

    #[test]
    fn step_complete_for_optional_checks_requires_either_check() {
        let neither = GuidedWorkflowProgress {
            build_done: true,
            ..Default::default()
        };
        assert!(!neither.step_complete(GuidedWorkflowStep::OptionalChecks));

        let with_iso9660 = GuidedWorkflowProgress {
            build_done: true,
            iso9660_done: true,
            ..Default::default()
        };
        assert!(with_iso9660.step_complete(GuidedWorkflowStep::OptionalChecks));
    }

    #[test]
    fn flow_complete_only_requires_build_done() {
        let p = GuidedWorkflowProgress {
            build_done: true,
            ..Default::default()
        };
        assert!(p.flow_complete());
        let q = GuidedWorkflowProgress::default();
        assert!(!q.flow_complete());
    }
}
