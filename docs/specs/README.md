# Specs

Concrete proposals for non-trivial changes, per ADR 0009. Each spec answers:

- **What** — the change in one sentence
- **Why** — the user-visible problem or rubric criterion
- **Scope** — explicit list of what is and isn't covered
- **Non-goals** — adjacent things this won't do (defends against scope creep)
- **User-visible API** — flags, env vars, GUI widgets, CLI subcommands changed
- **Error model** — new EngineError variants + chaos test scenarios
- **Observability** — new tracing fields + log shape
- **Test plan** — unit + property + chaos + e2e items needed
- **Rollout** — feature flag → soak → promote → remove timeline

After landing, link the spec from the implementing PR description and from
any ADRs that document load-bearing decisions made during the spec.

`wip/` is for early brainstorm drafts that haven't reached PR stage.
