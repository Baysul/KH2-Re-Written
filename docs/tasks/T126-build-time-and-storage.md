# T126 — Reduce build time and compiler storage

Owner Codex, 2026-10-15. Gate P04 development workflow only. Prerequisites:
fidelity contract, milestones, unknowns, T125 and the user's request to reduce
fresh executable build time and the oversized debug directory. Build identity:
HEAD `3aa251e`, Rust/Cargo 1.99.0, x86_64-pc-windows-msvc, package 0.1.1 plus
the existing uncommitted work (preserved). Source identity: no installed game
read or gameplay session; source admission and all fidelity gates are unchanged.

Private evidence directory:
`%LOCALAPPDATA%/KH2FinalRewrite/logs/T126-build-optimization/`.
Verified release launcher/viewer and touched-file backups:
`%LOCALAPPDATA%/KH2FinalRewrite/backups/T126-build-optimization/`.

## Steps and evidence

1. Inventory target outputs, current changes, toolchain, running build processes
   and reparse points. `target-before.json`, `git-before.txt` and
   `preserved-runtime.json` retain exact private paths and hashes. Initial debug
   logical size: 38,849,351,123 bytes, including 18,846,957,568 PDB bytes and
   8,108,510,495 incremental-cache bytes. File totals can count hard links twice;
   they do not establish physical disk allocation.
2. Use compact file/line symbols for workspace development/tests and no symbols
   for registry/git dependencies, retaining incremental workspace compilation.
   Opt-in `dev-symbols` keeps workspace local/type inspection available.
3. Select Rust's bundled MSVC-compatible LLVM linker. Initial synthetic smoke
   rejected unstable `linker-flavor=msvc-lld`; use stable `linker=rust-lld.exe`,
   which successfully links and runs an authored synthetic Rust fixture.
4. Fix the stale `kh2fm` helper target to `KH2RW`, select just the launcher and
   its model/image viewer siblings, and expose `-Debug` / `-LauncherOnly`.
   `build.bat` uses that helper and retains its existing optional `skate3`
   feature; the helper's explicit `-Skate3` option preserves that build choice.
   No unrelated game source is opened. Release codegen/optimization rules stay
   unchanged. Timing reports are opt-in with `-Timings` so ordinary builds do
   not accumulate an HTML report each time; the measurement runs enable them.
5. Measure builds with Cargo timing reports, remove obsolete development outputs
   using Cargo's profile-scoped cleanup after checking the resolved target and
   process inventory, then run required verification and record final storage.
   The first dry run refuses the existing target's missing `CACHEDIR.TAG`;
   `clean-dry-run.log` retains that failure. After checking the resolved path,
   output inventory and absence of reparse points/running debug executables,
   restore the tag from a newly Cargo-built synthetic fixture. The successful
   `clean-dry-run-02.log`, `cleanup-safety.json` and `clean.log` retain the
   recovery and profile-scoped removal of 27,579 files / 36.2 GiB. Release
   outputs remain outside the cleanup scope.

The profile and target-selection semantics were checked against the pinned
compiler and the primary [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html),
[Cargo build](https://doc.rust-lang.org/cargo/commands/cargo-build.html) and
[rustc linker](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker)
documentation (reviewed 2026-10-15). Documentation is configuration evidence,
not a measured performance guarantee or game-build compatibility proof.

## Checks, uncertainty, verdict and next task

PASS initial synthetic bundled-linker fixture (`linker-smoke.log`) and compact
symbol panic/backtrace fixture (`backtrace-build.log`, `backtrace-smoke.log`):
the latter exits 101 as expected and reports the authored fixture's line two
in the main stack frame, not just the panic header (`backtrace-receipt.json`).
PASS helper PowerShell parse, formatting (`fmt.log`), progress consistency
(`progress-check.log`) and all nine synthetic progress tests (`progress-tests.log`).

Observed launcher-only builds (`baseline-release.{log,json}`,
`cold-debug-launcher.{log,json}`): the existing-cache release command rebuilds
dependencies and takes 600.71 seconds; an empty `target/debug/` build with the
new settings takes 371.93 seconds including helper startup (Cargo reports
6m 09s). Downloads were cached; these builds partly overlapped, used different
optimization profiles and are not a controlled before/after speedup comparison.
The fresh development launcher cache is 3,400,547,729 logical bytes (3.17 GiB).
The final all-workspace footprint must be measured separately below.

`cargo-timing-20261015T213310934Z-ac6e7fdaafdb787e.html`,
`cargo-timing-20261015T213742055Z-ac6e7fdaafdb787e.html` and
`slowest-units.json` retain timings. Bevy PBR is the longest individual unit:
239.25 seconds in that release run and 113.71 seconds in that development run.
Concurrent units overlap; their durations must not be summed as elapsed time.
No controlled linker-only or symbol-only speedup, gameplay FPS improvement,
whole-game equivalence or final gate acceptance is inferred.

PASS locked warnings-denied workspace/all-targets Clippy (`clippy.log`), all
556 test executions (`tests.log`), locked workspace build (`build.log`) and
180-frame source-free shell smoke (`smoke.log`). The smoke retains the existing
default-controller-mapping warning. `checks.jsonl` records exit codes and elapsed
times, including any build-lock waits; all four commands exit zero.

PASS development helper with the batch entry point's existing feature and both
viewers (`debug-bundle.{log,json}`): 89.33 seconds using cached workspace
dependencies. A timestamp-forced launcher rebuild takes 21.93 seconds including
the helper (Cargo 19.87s), and an unchanged build takes 2.80 seconds (Cargo 1.43s):
`warm-launcher-rebuild.{log,json}`, `warm-noop.{log,json}`. The benchmark changes
only the launcher's source timestamp, restores it and checks identical source
bytes. It does not measure arbitrary semantic edits. These commands share the
machine with release compilation; no universal build-time guarantee is claimed.

PASS waited development launcher `--version`, exit zero, `KH2RW 0.1.1`, and
exact executable/product-copy hash equality (`debug-receipt.json`,
`debug-version{,.stderr}.log`). A direct GUI-executable invocation initially
used an unreliable/stale PowerShell `$LASTEXITCODE`; the final check explicitly
waits for the process and reads its actual exit code. This was a check-harness
failure, not evidence of an application failure. Lockfile and benchmark source
diffs remain empty.

After the full workspace checks, both developer feature variants, the bundled
viewers and the warm rebuild, `debug-after.json` measures 10,658,485,459 logical
bytes (9.93 GiB): **72.56% below** the initial 38,849,351,123 bytes (36.18 GiB).
PDBs fall from 18,846,957,568 to 2,676,604,928 bytes; incremental caches from
8,108,510,495 to 2,623,102,482 bytes. Useful caches remain in place. This is not
merely a post-clean empty-directory measurement and does not promise a permanent
size ceiling. Physical allocation can differ because of hard links.

PASS initial optimized helper with the existing batch feature and both viewers
(`release-launcher.{log,json}`), 776.52 seconds including helper startup (Cargo
12m 55s). This run shared the machine with verification and developer builds;
it does not establish a sub-ten-minute release build or a controlled speedup.

For an isolated cold release measurement, verify and preserve the newly built
runtime siblings outside the checkout (`isolated-runtime-backup.json`), check
resolved paths/reparse points/processes again, and use Cargo's release-scoped
dry run and clean (`release-clean{-dry-run,}.log`). That cleanup removes 3,313
files / 3.5 GiB of release outputs only, including obsolete dependency variants;
the verified debug cache remains intact. Rebuild the identical three targets
and batch feature without another Rust build running. Downloads remain cached.

PASS isolated cold optimized launcher and both viewers with the preserved batch
feature: **579.18 seconds including the helper**, Cargo **9m 38s**.
`isolated-cold-release.{log,json}` records zero exit and a 2,614,366,405-byte
release cache. This starts with an empty release directory and cached source
downloads; it is a local observation, not a universal cold-build ceiling or a
controlled before/after comparison with the earlier contended baseline.

PASS final helper parse and actual cached release/development invocations with
and without timing reports (`final-helper-{default,timings,debug}.log`). The
default creates no timing report; `-Timings` creates one. PASS waited release
`--version` with zero exit, version `KH2RW 0.1.1`, all three runtime files and
exact Cargo-executable/launcher-copy hash equality (`release-receipt.json`,
`release-version{,.stderr}.log`). Both sets of outside-checkout runtime backups
still match their original receipts.

PASS task-index regeneration, whitespace and changed-file provenance review.
The changes are authored Cargo configuration, a maintained build helper and
public workflow documentation. No dependency, lockfile change, game-source
read/write, user-save write, commit, push or publication is introduced. The
coverage ledger is unchanged: no reviewed game-function/behavior evidence or
final gate acceptance changed. Arbitrary code-edit timings, actual gameplay FPS,
installed comparisons and second-machine compatibility are NOT RUN here.

Verdict: PASS for measured local storage reduction, functioning compact builds
and the tested build workflow. The observed isolated cold optimized package is
under ten minutes, while ordinary cached development rebuilds avoid recompiling
the engine; this does not pass a final roadmap gate or guarantee every edit's
duration. Next: retain the caches for everyday edits, use the development helper
for iteration and measure an actual runtime workload before claiming FPS gains.
