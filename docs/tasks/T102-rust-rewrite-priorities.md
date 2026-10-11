# T102 — Rank essential Rust rewrite work

Owner: Codex. Date: 2026-10-10. Gate: planning support for P00–P31; no gate advancement.

## Prerequisites and identity

Read the fidelity contract, milestones, unknowns and latest task T101 before planning. Reviewed current-state, architecture, workspace/module inventory and relevant recent tasks. Checkout: `f248223384d5012c69739bce39d5a12c77834e99`. Installed source/build validation: NOT RUN; no game files read and no gameplay session started. Build 15194255 is historical repository context only.

## Steps and evidence

1. Compared roadmap requirements with newer implementation summaries and selected Rust modules.
2. Distinguished existing partial implementations from missing runtime work and unresolved fidelity checks.
3. Authored `docs/rust-rewrite-priorities.md`, ranking fourteen systems and proposing a saveable story segment.

Exact review evidence: `docs/fidelity-contract.md`, `docs/milestones.md`, `docs/unknowns.md`, `docs/CURRENT-STATE.md`, `ARCHITECTURE.md`, `Cargo.toml`, `docs/tasks/T080-launcher-and-session-check.md`, `docs/tasks/T100-merge-work-branches.md`, `docs/tasks/T101-skate3-board.md`, `crates/player_sim/src/lib.rs`, `crates/area_sim/src/lib.rs`, `crates/event_sim/src/lib.rs`, `crates/package_reader/src/probe.rs`. Further feature records are linked in the deliverable. No extracted evidence or private paths added.

## Checks

- Required reading and repository inventory: PASS.
- Markdown relative links: PASS (all deliverable link targets exist). Patch whitespace: PASS (`git diff --check`). Task index regenerated with `tools/New-TaskIndex.ps1`.
- Cargo fmt/clippy/test/build and synthetic smoke: NOT RUN (documentation-only assessment; no runtime changes).
- Installed behavior and full-game equivalence: NOT RUN.

## Uncertainty

Older roadmap summaries lag recent tasks. Presence of code and prior task claims do not independently establish fidelity. Existing future-dated records were read as written. Ranking is advisory dependency/impact judgment. Untracked `crates/skate_source/examples/` was present before this task and is left untouched.

## Verdict

PASS for the documentation deliverable only; no gameplay or source-qualification gate passes.

## Next task

Select one evidenced story route and enumerate its simulation, encounter, progression and save prerequisites, then implement and compare that bounded route without inventing missing rules.
