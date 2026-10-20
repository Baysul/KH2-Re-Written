# T193 — Resolve Linux/Epic rebase conflicts without losing upstream work

Owner: coding agent. Gate: development integration only; no source, platform,
gameplay or fidelity gate advances. Prerequisites: T177–T181 and the incoming
world-map/combat work. Requested work: resolve the four conflicts in the paused
`epic-linux` rebase, without creating a commit or advancing the rebase.

## Build and source identity

Upstream base: `3645961`; replayed Linux/Epic commit: `3a3f5c3`. Rust/Cargo
1.99.0, Bevy 0.19.1, existing dependency pins and workspace members retained.
Only code and synthetic checks are used; no installed game source or save is
needed or changed.

## Evidence and steps

Private bundle **T193-B01**, outside the checkout, holds `status-before.txt`,
`unmerged-before.txt`, `conflicts-before.diff`, both parent lockfiles,
`lock-verification.json`, `checks.sh`, `check-results.txt`, `check-*.log`,
`test-summary.json`, `enemy-vs-upstream.diff` and `unmerged-after.txt`.

1. Inspect Git's three-way conflict entries and both sides of each conflict.
2. `.gitignore`: retain both gameplay-example and audio-example allowances.
3. `source/main/src/launcher.rs`: export both `landing_room_config` and
   `launch_file_candidates`, preserving the other shared launcher helpers.
4. `source/visual/src/lib.rs`: register both `window_clear` and `world_map_menu`.
5. `Cargo.lock`: retain the upstream `world_map` package alongside the port's
   added packages, in package-name order. Do not regenerate or refresh the lock.
6. Parse both parent lockfiles and the result. The result contains exactly the
   union of their package identities (445 upstream, 512 port, 513 merged), with
   unchanged versions, sources and checksums. Resolve dependency references to
   package identities: every merged dependency edge equals the union of the
   parents' edges, including the `main` and `visual` world-map dependencies.
7. Run the existing workspace checks, then stage only the explicit resolutions
   and this record. Leave the rebase paused for the user.

## Checks

| Check | Result |
| --- | --- |
| Lock package identities, metadata and dependency-edge union | PASS |
| `cargo fmt --all --check` | PASS |
| `git diff --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | FAIL: two enemy weapon-trap tests |
| Full suite with `--no-fail-fast`, no tests skipped | FAIL: 895 passed, 2 failed, 1 ignored, 30 test programs |
| `cargo build --workspace --locked` | PASS |
| Conflict markers and unmerged index entries absent | PASS: no markers; zero unmerged entries; rebase still paused |
| `python3 -B tools/progress.py --check` and its existing synthetic tests | PASS: report unchanged; 9 tests |
| Native GUI, audio, source qualification and Windows runtime | NOT RUN: outside this integration check |

## Uncertainty, verdict and next task

**Conflict resolution complete and staged; integration verification FAIL.** No
commit is created and the rebase is not advanced. No test is added, changed or
skipped. Compilation and synthetic tests cannot establish native runtime or
source compatibility.

The two failures are
`traps::weapon::tests::detaching_and_unsyncing_free_the_shield_and_attaching_takes_it_back`
and
`traps::weapon::tests::the_bone_point_falls_back_to_the_object_and_nothing_is_a_summon`.
Both panic with `the trap is translated` at
`source/game/enemy/src/traps/weapon/tests.rs:34:36`. The complete enemy package
matches upstream base `3645961` exactly (`enemy-vs-upstream.diff` is empty).
These unchanged upstream files are not edited as part of conflict resolution;
the pristine base is not separately built. The first test run stopped at this
failure; the second used `--no-fail-fast` to check all remaining suites too.

Next: investigate the enemy trap lookup failures separately. The user retains
control of whether and when to continue the paused rebase.
