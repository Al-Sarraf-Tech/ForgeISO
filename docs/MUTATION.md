# Mutation Testing — ForgeISO

ForgeISO uses [`cargo-mutants`](https://mutants.rs/) to validate that the
engine's test suite actually catches behavioural regressions in the
high-blast-radius modules (config validation, autoinstall YAML
generation/merging). A passing test suite proves the code does not crash;
a high mutation kill score proves the tests would notice if the code
started lying.

## Scope

Mutation testing is currently scoped to two recently-decomposed surfaces:

- `engine/src/config/inject/**/*.rs` — `InjectConfig` and the per-concern
  validators (identity / system / packages / network / ssh / storage /
  grub / output).
- `engine/src/autoinstall/ubuntu/**/*.rs` — `generate_autoinstall_yaml`
  and `merge_autoinstall_yaml` (the highest-risk YAML producers).

Test files (`**/tests/**`, `**/tests.rs`) are excluded by config so we
never mutate the test code itself.

The full configuration lives at `.cargo/mutants.toml`.

## Threshold

Kill-score floor: **80 %** (configurable via
`FORGEISO_MUTANTS_THRESHOLD`).

A mutant is counted as *killed* when it is `caught` (a test failed),
`timeout` (the test loop spun forever — production code is sensitive to
this change), or `unviable` (the mutant didn't compile — also evidence
the source carries meaningful semantics). A mutant is counted as *missed*
only when the full test suite passed against it.

Score formula:

    kill_score = (caught + timeout + unviable) / tested * 100

CI fails when `kill_score < threshold`. The threshold is *gating-aware*:
it represents a regression floor, not an absolute bar. Ratchet upward
over time by raising `FORGEISO_MUTANTS_THRESHOLD` only after a clean run
demonstrates the higher number is achievable.

## How to Run

### Full run (slow — minutes to an hour)

```bash
scripts/run-mutants.sh
```

This runs `cargo mutants --in-place --baseline=skip` against the scoped
modules, then checks the kill score against the threshold. The full log
is saved under `${FORGEISO_LOG_DIR:-/mnt/nvmeINT/logs}/forgeiso-mutants.*.log`.

### Compile-only smoke (fast)

```bash
scripts/run-mutants.sh --check-only
```

Generates and compiles every mutant but skips the test phase. Confirms
the configuration is syntactically valid and the mutants build.

### Differential run (PR / pre-push)

```bash
scripts/run-mutants.sh --in-diff origin/main
```

Only tests mutants that touch lines changed since `origin/main`. This
is the recommended pre-push gate — typically completes in seconds.

### Sharded run (CI parallelism)

```bash
scripts/run-mutants.sh --shard 1/4
```

Splits the mutant set into four parts and runs the first. Use with
matrix CI to fan-out across runners.

## Triaging a New Surviving Mutant

When CI reports a regression (kill score dropped or a specific mutant
escaped), follow this loop:

1. **Reproduce locally** — `scripts/run-mutants.sh --in-diff origin/main`
   (or full run if the survivor is from older code). The log lists each
   missed mutant with file/line/before/after.

2. **Read the mutant** — the report line looks like:

       engine/src/config/inject/validate/identity.rs:42:13: replace == with !=

   Open that line and ask: *if I made this change in production, what
   real-world failure would it cause?* If the answer is "none — the test
   shouldn't care", suppress the mutant via:

       // mutants: skip — comparison only matters for IPv6 short form
       if a == b { ... }

   Use sparingly. Document why in the comment.

3. **Add a kill test** — usually the cheapest fix. Write a unit test in
   the corresponding `engine/src/config/inject/tests/<concern>.rs` (or
   `engine/src/autoinstall/ubuntu/tests.rs`) that exercises the exact
   branch the mutant attacks. The test name should describe the
   *behavioural assertion*, not the mutant:

       #[test]
       fn validate_locale_rejects_blank_string() { ... }

4. **Re-run** — `scripts/run-mutants.sh --in-diff HEAD~1` to confirm the
   new test kills the mutant.

5. **Commit** — conventional commits format, single concern per commit:

       test(inject): kill mutant — locale blank-string validator

## Common Surviving-Mutant Patterns

These show up repeatedly in YAML/validator code; tactics for each:

| Pattern | Tactic |
|---|---|
| Boolean-flag default flips (`unwrap_or(true)` -> `unwrap_or(false)`) | Add a test asserting the default-value behaviour explicitly. |
| String-literal substitutions (`"lvm"` -> `""`) | Snapshot test the rendered YAML; assert the literal appears. |
| Comparison swaps (`>` -> `>=`) on len checks | Add boundary tests at `n - 1`, `n`, `n + 1`. |
| `Result::Ok(())` injection on validators | Add a test that feeds bad input and asserts `Err`. |
| Loop-body deletion in `for` over packages/repos | Add a test asserting *every* element is processed (e.g. count, sort, dedup result). |

## Why Not 100 %?

Mutation testing surfaces equivalent mutants — semantically identical
code with a different syntax that no test could ever distinguish. Common
sources:

- Allocation-pattern changes (`Vec::new()` vs `Vec::with_capacity(0)`).
- Unreachable error-handling branches the type system already rules out.
- Tracing/log statements with no observable effect.

The 80 % floor leaves head-room for these without forcing test churn
that doesn't improve real coverage.

## Operator Cheat-Sheet

```bash
# install (one-shot)
cargo install cargo-mutants --locked

# full run + threshold gate (slow)
scripts/run-mutants.sh

# compile-only (fast smoke)
scripts/run-mutants.sh --check-only

# pre-push diff gate
scripts/run-mutants.sh --in-diff origin/refactor/major-gui-overhaul

# raise the bar (after a clean run shows the new floor is achievable)
FORGEISO_MUTANTS_THRESHOLD=85 scripts/run-mutants.sh
```
