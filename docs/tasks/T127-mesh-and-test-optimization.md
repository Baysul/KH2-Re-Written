# T127 — Reuse animated mesh storage and omit empty CLI harnesses

Owner Codex, 2026-10-15. Gates P04 build workflow / P14 presentation engineering
only; no final game acceptance. Prerequisites: fidelity contract, milestones,
unknowns, T126 and the user's request for further optimization. Build identity:
HEAD `3aa251e`, package 0.1.1, Rust/Cargo 1.99.0 x86_64-pc-windows-msvc plus the
preserved uncommitted T125/T126 work. Source identity: no installed game read or
gameplay session in this task; source admission remains unchanged.

Private evidence directory:
`%LOCALAPPDATA%/KH2FinalRewrite/logs/T127-mesh-and-test-optimization/`.
Backups of touched files and the existing local runtime belong outside Git:
`%LOCALAPPDATA%/KH2FinalRewrite/backups/T127-mesh-and-test-optimization/`.

## Steps and exact evidence

1. Inspect T126 timing reports and the current animation presentation callers.
   The broad Rust renderer dependencies dominate cold compilation; retain their
   features and optimization settings. Review twenty CLI targets with no local
   test/module declarations against T126's zero-test harness results.
   `empty-harness-review.json`, `metadata-after.json` and `git-before.txt` retain
   the private inventory and target configuration.
2. Set `test=false`, `bench=false` only for those twenty test-free CLI adapters.
   Keep all library tests, the launcher tests and model-inspector tests enabled.
   Normal workspace builds and all-target Clippy still compile these programs;
   future tests added to a declared adapter must re-enable its harness.
3. Reuse the existing Float32x3 position allocation for fixed-topology animated
   meshes, retaining ordinary `Assets::get_mut` change tracking and the renderer's
   upload path. Apply it to Sora, Keyblade, ordinary enemies, gallery enemies,
   townspeople, talk actors, map objects and the camp member preview. Preserve
   every existing coordinate calculation, selection, draw order and tick rule.
   Variable-topology effect construction stays on its existing path.
4. Check exact packed renderer bytes against the previous replacement operation,
   pointer/capacity retention, shrink/empty/refill, absent storage and growth.
   Generated cases include signed zero, subnormals and NaN payloads; the helper
   copies presentation data without changing validation or arithmetic.
5. Add a CPU-only generated-data benchmark with alternating path order and a
   discarded warm-up round. Compare replacement versus retained storage in one
   process, verifying nonempty and exactly equal packed vertex buffers. No
   installed source, GPU window or game data is needed.
6. Investigate an existing synthetic fixture cleanup failure during the first
   full test run (`tests.log`). The generated fixture name depended only on PID
   and wall-clock nanoseconds, which need not be unique across concurrent calls.
   Add an atomic process-local suffix in test-only code. A deterministic
   repeated-clock fixture test drops one fixture and verifies that the second
   remains valid. Production source admission code is unchanged. The original
   failed test passes alone (`probe-rerun.log`); a timestamp collision is
   consistent with the observed missing-directory cleanup failure, not a
   separately captured runtime trace of its cause.
7. Compare actual test-case identities with T126:
   `test-preservation-accepted.json` retains all 556 prior cases plus three new
   regression cases, with no missing cases, in the accepted implementation.
8. Direct deletion of forty obsolete CLI test executables/PDBs was rejected by
   automatic approval review with only "blocked by policy" as its reason; no
   shell deletion occurred. Use Cargo's own development-profile cleanup for
   the three owned packages instead, after verifying target/reparse/process
   bounds and preserving release runtime copies (`runtime-backup.json`).
   `cargo-clean-dry-run.log` and `cargo-clean.log` retain the package-scoped
   cleanup; the actual removal is 14,327 files / 6.7 GiB. Third-party renderer
   dependencies and release outputs remain cached; regenerate current development
   outputs through the mandatory checks. The old twenty harnesses stay omitted
   from ordinary future test runs.

The actual pinned `bevy_mesh` 0.19.1 implementation was inspected: attribute
replacement and mutable access operate on the same retained attribute values;
neither automatically updates entity AABBs. The packed-byte checks use its
`create_packed_vertex_buffer_data` API. The first Clippy attempt failed because
the provisional verification code named an older/nonexistent API; `clippy.log`
retains that failure, which was corrected before final checks. Primary
[Cargo target documentation](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-test-field)
and [Vec storage documentation](https://doc.rust-lang.org/std/vec/struct.Vec.html)
were reviewed alongside the pinned local code; they are not game compatibility
evidence.

## Checks, uncertainty, verdict and next task

The preliminary short `clear`/`extend` experiment (`mesh-bench-0{1,2,3}.json`)
has noisy/mixed timings and is not a CPU-speedup acceptance. Select whole-range
`Vec::splice` reuse after a longer same-process comparison, with 20,000 updates
of 12,288 generated indexed vertices per sample, one discarded warm-up and
eight measured samples per path in alternating order.

PASS `mesh-bench-long-0{1,2,3}.json`: replacement median / reuse median in
milliseconds are 253.147 / 210.671, 255.677 / 208.459 and 253.214 / 210.891.
These three runs measure 16.71–18.47% less CPU time in this vertex-update kernel,
with identical nonempty packed vertex bytes and retained 12,288-vertex capacity.
`mesh-bench-long-build.log` records the optimized build and execution. The fixture
is generated and source-free; this does not establish gameplay FPS or an
installed comparison. Allocation reuse is also checked independently by the
mesh regression's pointer/capacity and exact-byte assertions.

PASS final required checks, recorded in `checks.jsonl` and the corresponding
`*-accepted.log` files: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` (559 cases),
`cargo build --workspace --locked`, and
`cargo run -p windows_shell --locked -- --frames 180`. The source-free smoke
retains its default-controller-map warning. Initial failed attempts above are
retained; they are not counted as passing runs.

PASS local runtime delivery: `release-launcher.log` records
`tools/Build-Launcher.ps1 -Skate3` completing the cached optimized launcher and
both viewers in 57.99 seconds of Cargo time. `runtime-final.json` records a
nonempty launcher/viewers, an identical hash for the launcher copy and its build
artifact, and `KH2RW 0.1.1` with exit 0 from `--version`. This is a cached build,
not a new cold-compilation measurement.

PASS `debug-final.json`: after all development checks regenerated current
outputs, `target/debug` contains 9,640,611,689 logical bytes (8.98 GiB), including
2,490,970,112 PDB bytes and 1,852,837,426 incremental bytes. All forty old empty
CLI harness executables/PDBs are absent. Compared with T126's checked final
10,658,485,459 bytes, this is another 0.95 GiB reduction; compared with the
original 38,849,351,123 bytes, the total reduction is approximately 75.2%.
This is logical file size, not a claim about physical OneDrive allocation.

PASS `git diff --check`, unchanged `Cargo.lock`, `python tools/progress.py --check`
and all nine synthetic progress tests. Reviewed the presentation replacements
against this task's private pre-edit backups: all eight preserve their original
position mapping. Public changes contain source code, generated benchmark inputs
and process documentation; private runtime hashes, backups and logs remain
outside Git. Existing uncommitted work is preserved. No coverage ledger change,
source admission change, installed-source access, commit, push or publication.

NOT RUN: controlled installed/rewrite gameplay performance or behavioral
comparison. The CPU kernel result does not establish whole-game FPS, whole-game
equivalence or final gate acceptance; cold dependency compilation remains a
separate cost documented in T126.

Verdict: PASS for local engineering optimization and required checks only.
Next: use a source-validated controlled gameplay profile to identify the next
runtime bottleneck before changing simulation or renderer behavior.
