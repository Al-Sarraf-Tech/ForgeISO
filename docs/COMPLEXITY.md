# Code Complexity Gate — ForgeISO

ForgeISO uses clippy's built-in `cognitive_complexity` lint as the
workspace-level complexity gate. The threshold is pinned in
[`clippy.toml`](../clippy.toml) so the bar cannot drift between clippy
releases.

We chose `cognitive_complexity` over external tools (`scc`, `tokei`,
`radon-rs`, `cargo-complexity`) because it is:

- **Already in the toolchain** — no new dev dependency, no extra CI step.
- **Per-function granular** — flags exactly the function that needs
  decomposition, not just files or modules.
- **Score-based, not LOC-based** — penalises nested control flow,
  short-circuiting boolean chains, and `?` propagation rather than line
  count alone (which would punish inline tests and detailed comments).
- **Wired into the existing gate** — `cargo clippy --workspace
  --all-targets -- -D warnings` already turns every clippy warning into
  a hard CI failure, so simply enabling the lint via `clippy.toml` is
  sufficient.

## Threshold

```toml
# clippy.toml
cognitive-complexity-threshold = 25
```

The value 25 mirrors clippy's upstream default. We pin it explicitly so
that a future clippy release that tightens or relaxes the default does
not change ForgeISO's PR-pass criteria silently.

A function whose cognitive-complexity score exceeds **25** triggers
`-D warnings` and fails the lint job.

### What the score means

The lint counts (informally):

- `+1` for every nested control-flow construct (`if`, `match`, `for`,
  `while`, `loop`).
- An additional `+1` per level of nesting (an `if` inside an `if` costs
  3, not 2).
- `+1` per short-circuiting boolean operator in a chain (`a && b && c`
  is cheaper than `a && b && c && d`).
- `+1` per `?` propagation step.

At ~25, the lint generally fires on functions with ≥3 nested loops, or
≥4 levels of `match`/`if` nesting, or ~10 chained boolean conditions.
This is the "needs to be split into helpers" line for ForgeISO.

## How to Run

The complexity gate runs as part of the standard lint job:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

To see only complexity diagnostics during development:

```bash
cargo clippy --workspace --all-targets -- \
  -A warnings \
  -W clippy::cognitive_complexity \
  -W clippy::too_many_arguments \
  -W clippy::too_many_lines \
  -W clippy::type_complexity
```

To list every function ranked by score (informational, not gating):

```bash
cargo clippy --workspace --all-targets --message-format=json -- \
  -W clippy::cognitive_complexity 2>/dev/null \
  | jq 'select(.message.code.code == "clippy::cognitive_complexity") |
        {file: .target.src_path, msg: .message.message}'
```

## Refactor Recipes

When a function exceeds the threshold, decompose it. The four most
common patterns:

### 1. Extract a per-concern validator

If the function is a `validate_X` that checks several independent
properties, extract one helper per property.

Before (score ~30):

```rust
pub fn validate_inject_config(cfg: &InjectConfig) -> EngineResult<()> {
    if cfg.identity.username.is_empty() { return Err(...); }
    if cfg.identity.password.len() < 12 { return Err(...); }
    if cfg.system.timezone.contains(' ') { return Err(...); }
    if cfg.system.locale != "en_US.UTF-8" && cfg.system.locale != "C.UTF-8" {
        return Err(...);
    }
    for repo in &cfg.packages.apt_repos {
        if repo.starts_with("ppa:") && !cfg.is_ubuntu_like() {
            return Err(...);
        }
    }
    // ... etc
    Ok(())
}
```

After (score ~6 each):

```rust
pub fn validate_inject_config(cfg: &InjectConfig) -> EngineResult<()> {
    validate_identity(&cfg.identity)?;
    validate_system(&cfg.system)?;
    validate_packages(&cfg.packages, cfg.is_ubuntu_like())?;
    Ok(())
}

fn validate_identity(id: &Identity) -> EngineResult<()> { ... }
fn validate_system(sys: &System)     -> EngineResult<()> { ... }
fn validate_packages(pkgs: &Packages, ubuntu_like: bool) -> EngineResult<()> { ... }
```

This is the pattern already used by
`engine/src/config/inject/validate/{identity,system,packages,...}.rs`.

### 2. Lift a nested loop into an iterator chain

Before (score ~28):

```rust
for repo in &cfg.packages.apt_repos {
    for arch in &cfg.packages.architectures {
        if repo.contains(arch) {
            for pkg in &cfg.packages.requested {
                if pkg.starts_with(arch) {
                    out.push(format!("{repo}|{arch}|{pkg}"));
                }
            }
        }
    }
}
```

After (score ~10):

```rust
let triples = cfg.packages.apt_repos.iter()
    .cartesian_product(&cfg.packages.architectures)
    .filter(|(repo, arch)| repo.contains(arch))
    .flat_map(|(repo, arch)| matching_packages(arch, &cfg.packages.requested)
        .map(move |pkg| (repo, arch, pkg)));
```

### 3. Replace a deep `match` with a small dispatch table

If `match` arms are doing similar work with different constants, hoist
the constants into a slice and iterate.

### 4. Split a state machine across helper methods

If the function is one big `match self.state { ... }`, give each state
arm its own `fn handle_<state>(&mut self) -> EngineResult<NextState>`.

## When to Suppress

Sparingly. Suppression is allowed only with a one-line `// reason: ...`
comment on the same `#[allow(...)]`:

```rust
#[allow(clippy::cognitive_complexity)] // reason: generated match arm; splitting hides exhaustiveness
fn handle_distro_event(&mut self, evt: DistroEvent) -> EngineResult<()> {
    match evt {
        DistroEvent::Ubuntu24Detected => { ... }
        DistroEvent::Ubuntu22Detected => { ... }
        // ... 30 more arms ...
    }
}
```

Acceptable suppression reasons:

- **Generated match exhaustiveness** — splitting hides the
  `non_exhaustive_patterns` check the compiler does for us.
- **Hot-loop inlining requirement** — extracting helpers would force
  the optimiser to decide between two indirect calls and prevent inline
  expansion (rare; verify with `#[inline]` and a benchmark before using
  this excuse).
- **Spec-mirroring control flow** — the function's structure mirrors a
  spec section verbatim and rearranging it would obscure the
  spec-to-code mapping (provide a docs link).

Refactoring is always preferred. Suppression without a `reason:`
comment fails review.

## Operator Cheat-Sheet

```bash
# Run the gate exactly the way CI does
cargo clippy --workspace --all-targets -- -D warnings

# Find every offending function (informational)
cargo clippy --workspace --all-targets -- -W clippy::cognitive_complexity 2>&1 \
  | grep -B1 'cognitive complexity'

# Raise the bar after a clean run shows the new floor is achievable
# (edit clippy.toml; commit with an ADR explaining the new value).
sed -i 's/cognitive-complexity-threshold = 25/cognitive-complexity-threshold = 20/' clippy.toml
cargo clippy --workspace --all-targets -- -D warnings   # verify still clean
```

## Orchestrator Wiring

`.github/workflows/ci-rust.yml` is regenerated by
`haskell-ci-orchestrator`. The complexity gate piggybacks on the
existing `lint` job — clippy already runs `-D warnings`, and `clippy.toml`
is auto-discovered from the workspace root. **No workflow changes are
needed for this gate to take effect.**

If a future workflow regeneration ever drops the `-D warnings` flag,
re-add it via the orchestrator's `lint.extra_flags` setting; do not edit
the generated file directly.
