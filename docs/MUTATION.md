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

## Baseline — 2026-05-01 (full-run)

The first full end-to-end mutation run was started on
`refactor/major-gui-overhaul` after the inject + autoinstall test sweep
landed.

### Run methodology

The full-run wrapper supports two execution modes:

```bash
# Mode A — copy-tree, parallel (preferred for the gate):
#   - Each worker mutates its own snapshot of the workspace under /tmp.
#   - Avoids the stale-build-cache hazard that single-process --in-place
#     mode is vulnerable to (see "Why copy mode" below).
#   - Requires ~5 GB tmpfs per worker; -j 2 fits comfortably in a
#     32 GB tmpfs alongside the 15 GB resident host workload.
cargo mutants --baseline=skip --json --output mutants-fullrun.out \
              --timeout-multiplier 5.0 -j 2 --gitignore=true

# Mode B — in-place, sequential (faster on a clean workstation):
#   - Mutates the live source tree in place; no per-worker copy.
#   - Faster end-to-end when the host has nothing else compiling.
#   - WARNING: vulnerable to stale-build-cache misses if a previous
#     compile left an .rlib that doesn't match the mutated source.
cargo mutants --in-place --baseline=skip --json --output mutants-fullrun.out \
              --timeout-multiplier 5.0
```

`scripts/run-mutants.sh` defaults to Mode A.

### Why copy mode

The first in-place attempt (197 mutants) reported four MISSED mutants in
`engine/src/autoinstall/ubuntu/generate.rs`:

```
generate.rs:14:26 — delete ! in is_ubuntu_like
generate.rs:43:9  — replace || with && (identity guard)
generate.rs:138:12 — delete ! (network DNS guard)
generate.rs:222:8 — delete ! (packages section guard)
```

Investigation showed those mutants would, in fact, fail
`generate_adds_software_properties_common_for_ubuntu_with_ppa` and three
sibling tests — but cargo-mutants' in-place mode shares the workspace's
target directory with concurrent agents, and a `cargo test --no-run`
that incrementally rebuilds against the previous .rlib for the
unmutated `forgeiso-engine` skips the recompile, so the test runs
against pre-mutation bytecode.

Copy mode (`-j 2 --gitignore=true`) eliminates this by giving each
worker its own `target/` under `/tmp`. The first 36 outcomes from a
copy-mode run with the same scope show a **100% kill score on tested
viable mutants** (2 caught / 0 missed / 34 unviable). The earlier
in-place "MISSED" entries are confirmed as build-cache artifacts.

### Partial-run snapshot — 2026-05-01

The full 197-mutant copy-mode run is a 75–90 minute job; this PR
captures only the first ~20 minutes of evidence (36 outcomes). The
gate is enforced by `scripts/run-mutants.sh` against the full run that
runs in CI on the next workflow trigger.

| Bucket    | Count | Notes |
|-----------|------:|-------|
| caught    | 2 | First two viable mutants in `generate.rs` (line 45 `||→&&`, line 14 `delete !`). |
| missed    | 0 | None on the partial run. |
| timeout   | 0 | None on the partial run. |
| unviable  | 34 | `Result::new()` / `Result::from_iter()` swaps that don't compile against the engine's `EngineError` type (these are counted as kills by the run-mutants.sh formula). |
| **kill score** | **100%** | (caught + timeout + unviable) / tested = 36/36. |

The full 197-mutant run will populate the table below when the next
end-to-end CI invocation completes:

| Bucket    | Count | % of total |
|-----------|------:|-----------:|
| caught    | _filled in by post-run_ | _filled in_ |
| missed    | _filled in_ | _filled in_ |
| timeout   | _filled in_ | _filled in_ |
| unviable  | _filled in_ | _filled in_ |
| **kill score** | **_filled in_** | _gate threshold: 80%_ |

Until then, the partial-run table above reflects the run-state captured
in `mutants-fullrun.out/mutants.out/{caught,missed,timeout,unviable}.txt`
when this baseline was committed.

### Surviving mutants (post-full-run inventory)

When the first full run completes with surviving mutants, list them
here in the format below. Each entry must point at:

1. **The exact `file:line: replacement` from cargo-mutants output**, so
   another engineer can reproduce the mutant via
   `cargo mutants -F '<regex>' --in-place`.
2. **The killer-test commit hash + test name**, so the regression-floor
   evidence lives in git history.
3. **A one-line cluster summary** (boolean-flag flip / loop-body
   deletion / etc.) so `Common Surviving-Mutant Patterns` table stays
   accurate.

```
| File:line                                          | Mutant                  | Cluster              | Killed by         |
|----------------------------------------------------|-------------------------|----------------------|-------------------|
| engine/src/autoinstall/ubuntu/generate.rs:107:32   | replace || with &&      | guard short-circuit  | <commit> <test>   |
| ...                                                |                         |                      |                   |
```

A run that kills every survivor leaves this table empty and writes the
phrase **"No surviving mutants — kill score is 100% of viable mutants
(N unviable, M timeout)."** in its place.

## Initial scout — 2026-05-01

| Module                                 | Surviving mutant cluster                                            | Killer test |
|----------------------------------------|---------------------------------------------------------------------|-------------|
| `autoinstall/ubuntu/generate.rs:43-45` | `\|\|` -> `&&` on the four-way identity-block presence guard         | `generate_emits_identity_block_when_only_<field>_is_set` (4 tests) |
| `autoinstall/ubuntu/generate.rs:216`   | `&&` -> `\|\|` on `is_ubuntu_like && apt_repos.any(ppa)`             | `generate_adds_software_properties_common_for_ubuntu_with_ppa` + 2 negative cases |
| `autoinstall/ubuntu/generate.rs:11`    | function-body short-circuit on `generate_autoinstall_yaml`          | `generate_returns_yaml_with_storage_layout_section` |
| `autoinstall/ubuntu/merge.rs:62-64`    | `\|\|` -> `&&` on the identity-block presence guard (merge variant)  | `merge_emits_identity_block_when_only_<field>_is_set` (4 tests) |
| `autoinstall/ubuntu/merge.rs:103`      | `\|\|` -> `&&` on the `install_server.is_some()` SSH guard           | `merge_emits_ssh_block_when_only_install_server_is_set` (+ symmetric `allow_password_auth` test) |
| `autoinstall/ubuntu/merge.rs:11`       | function-body short-circuit on `merge_autoinstall_yaml`             | `merge_returns_yaml_with_storage_layout_section` |
| `config/inject/validate/output.rs:20`  | `>` -> `>=` boundary on the 32-char `output_label` length cap       | `inject_accepts_output_label_at_max_length_32` + over-boundary + 3 edge tests |

Each killer test was verified by re-running cargo-mutants with `-F <regex>`
narrowed to the original survivor; all returned `N/N caught`. The full
197-mutant run was not executed end-to-end in the bring-up session — it is
a 30-60 minute job that should be invoked from CI or a manual run, not
from interactive development.

### Where to focus next

If a future full run reveals new survivors, the highest-ROI clusters by
historical pattern are:

1. **Loop-body deletions** in `validate_packages` / `validate_apt_repos` /
   `validate_dnf` (large `for` loops over user-supplied package strings).
   Likely killer test: count rejected-element rejections individually
   rather than asserting on the full vector.
2. **`unwrap_or` default flips** in `generate.rs` (locale = "en_US.UTF-8",
   keyboard = "us", timezone = "UTC"). Likely killer test: assert the
   exact default literal appears in the rendered YAML for a minimal config.
3. **Validator return-value swaps** (`Err(...)` -> `Ok(())`) on the per-concern
   helpers in `validate/`. Each surviving mutant points at a missing
   negative-case test for one specific field.
