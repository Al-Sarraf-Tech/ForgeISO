# ADR 0009: Spec-driven development workflow

- **Status**: Accepted
- **Date**: 2026-05-01

## Context

The S+ tier rubric (`~/.claude/TIER_RUBRIC.md`) Process dimension at S+ requires:

> brainstorm → spec → ADR → plan → review → implement workflow. Spec-driven
> development. Pilot-first rollouts. Per-cluster retros. Issue templates from
> checklists.

ForgeISO had grown organically — features landed via ad-hoc PRs, with the
mailing-list-style design discussion happening in commit messages. This works
at small scale; it stops scaling when the project has 8 ADRs, 705 tests, 13
chaos scenarios, and a contract-locked public API. Future contributors (human
or agent) need a documented path from "I want to add X" to "X is in main".

## Decision

Adopt a six-step workflow for any non-trivial change. "Non-trivial" means:
adds a new public API, adds a new dependency, changes user-visible behavior,
or touches >1 ADR.

Trivial changes (typo fixes, doc edits, internal refactors that preserve
public API and behavior) skip the workflow and go straight to a PR.

### The six steps

1. **Brainstorm** — short freeform write-up exploring the problem and 2-3
   approaches. Lives as a comment on the GitHub issue or in `docs/specs/wip/`
   if pre-issue. Goal: surface options before commitment.
2. **Spec** — concrete proposal answering: what, why, scope, non-goals,
   user-visible API, error model, observability, test plan. Lives at
   `docs/specs/<short-title>.md`. Goal: lock alignment before code.
3. **ADR** — required if the spec lands a load-bearing decision (storage
   layer, public API shape, dependency choice, error handling philosophy).
   Numbered sequentially in `docs/adr/000N-<title>.md`. Goal: future
   readers understand WHY, not just WHAT.
4. **Plan** — work breakdown of the spec into commits. Lives in the spec
   itself or as a separate `docs/specs/<short-title>-plan.md`. Goal: PR
   reviewer can map each commit to a plan step.
5. **Review** — at least one other engineer (or a designated review agent)
   reads the spec + plan before any code lands. Reviewer checks: does the
   spec match the brainstorm? does the plan match the spec? are alternatives
   acknowledged?
6. **Implement** — commits land per the plan. Each commit is conventional-
   format; commit message references the spec. The final PR description
   links the spec, the ADR (if any), and shows test evidence.

### Mapping to existing artifacts

- `docs/specs/` is new — created by this ADR. Existing planning docs (none
  in this repo today) would migrate here.
- `docs/adr/` already exists; this ADR adds it as the formal output of Step 3.
- The `scripts/s-tier-audit.sh` gate enforces that ADRs ≥5 + RUNBOOKS +
  COMPLIANCE + SLO + CHAOS + CHANGELOG exist; future enhancement: also
  check that spec files referenced in PR descriptions exist.

### Pilot-first rollouts

When a change has user-visible behavior, default to a rollout pattern:

1. Land the change behind a feature flag (CLI: `--experimental-X`; GUI:
   hidden in a `Settings → Experimental` panel; engine: `EXPERIMENTAL_X` env
   var or `BuildConfig.experimental: HashMap<String, Value>`).
2. Land docs + chaos test + property test for the new behavior.
3. Document in `CHANGELOG.md` under `### Unreleased / Experimental`.
4. Promote to default in the next minor version after one release of soak.
5. Remove the flag in the version after that.

## Alternatives considered

- **Skip the workflow entirely** ("we're a small team"): cheaper short-term
  but accumulates undocumented decisions that future contributors can't
  reconstruct. ForgeISO already has 8 ADRs precisely because past authors
  noticed this pattern — formalizing it stops the slide.
- **Heavier RFC process** (Rust-style RFC repo, 2-week comment period):
  too much for a 4-crate project. Spec + 1 reviewer is the right scale.
- **Git-only** (long commit messages doing the spec work): doesn't scale
  past ~5 paragraphs and isn't searchable. Markdown specs in repo are
  searchable, reviewable, linkable.

## Consequences

- **Positive**: every load-bearing decision has a paper trail. Onboarding
  doc (future) can point at `docs/specs/` + `docs/adr/` and let new
  contributors self-serve.
- **Positive**: PRs are smaller because the spec separates the "what" from
  the "how". Reviewers can sign off on the spec before any code review.
- **Positive**: agents (Claude / Codex / Gemini / local LLM) can read the
  spec format and produce conforming PRs.
- **Negative**: small fixes acquire process overhead if the "trivial vs
  non-trivial" line is unclear. Mitigate by keeping the trivial bypass
  generous (typo / docs / internal refactor → straight to PR).
- **Negative**: spec docs can drift from reality if not maintained. Mitigate
  by requiring spec updates as part of any commit that contradicts the spec.
- **Process consequence**: future ADRs follow the spec → ADR pipeline
  rather than appearing fully-formed. The first ADR of an area should
  reference the spec.

## Implementation reference

This ADR itself is the first artifact produced by Step 3 of the workflow.
The post-decomposition uplift work (Phase 1-7 commits, branch
`refactor/major-gui-overhaul`) was done without specs — the work was
mostly mechanical refactoring that the rubric demanded. Future
non-trivial changes should follow the workflow.

To bootstrap, the next non-trivial change should:

1. File a GitHub issue
2. Brainstorm in the issue
3. Land a spec at `docs/specs/<short-title>.md`
4. If the spec carries a load-bearing decision, land ADR 0010 alongside
5. Land the plan
6. Implement per the plan

This ADR will be cited as "follow the workflow" in the spec-template that
future contributors copy from.
