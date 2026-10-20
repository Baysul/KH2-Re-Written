# Build and verification baseline

## Identity, scope and verdict

Research child: **Establish a reproducible Linux and Windows verification baseline**; owner: delegated leaf researcher for `project_owner`. Research branch: `research/build_verification_baseline`; inspected base: `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`. Prerequisites: fidelity contract, milestones, unknowns, folder guide, T170, T171 and the live Wayfinder scope. This is planning evidence, not implementation or a gate admission.

**Verdict: PARTIAL / build proof BLOCKED.** The exact upstream dependency revision is anonymously reachable now, but the existing Cargo Git cache cannot supply it. Recovery was not performed and compilation was not retried. Formatting and the existing progress-report checks pass. Native Linux/Windows feature parity, hardware operation and selected-source comparisons remain **NOT RUN** in this research session. No source installation, save, application or GUI was accessed. Only this report was added; no configuration, dependency, test, ledger or workflow was changed.

## Toolchain and locked workspace

`rust-toolchain.toml:1-5` pins Rust **1.99.0**, the minimal profile, rustfmt, Clippy and the `x86_64-pc-windows-msvc` target. Bounded local probes confirmed rustc **1.99.0** (`b940084d7`, 2026-09-28), Cargo **1.99.0** (`5f94df478`, 2026-08-27), and installed Linux GNU and Windows MSVC targets. The observed host is Linux x86_64 with a Wayland session. These facts do not establish Windows SDK/linker libraries or native Windows execution. `.cargo/config.toml:1-4` selects `rust-lld.exe` for MSVC; `CONTRIBUTING.md:29-32` still requires Windows SDK/MSVC libraries.

The workspace contains fifteen members, including two skateboard-engine adapters (`Cargo.toml:1-18`); its package version is **0.1.2**, edition **2024**, minimum Rust **1.99** (`Cargo.toml:20-24`). Main marks the engine dependencies optional under `skate3`, but they remain workspace members (`source/main/Cargo.toml:21-24,72-73`). This explains why omitting that feature is not evidence that workspace dependency resolution avoids them. Do not delete members or rewrite the lockfile to manufacture a green KH2 check.

Main and visual explicitly pin Bevy **0.19.1** with defaults disabled (`source/main/Cargo.toml:30`, `source/visual/Cargo.toml:20`). The lock records Bevy **0.19.1**, wgpu **29.0.4**, winit **0.30.13** and windows **0.62.2** (`Cargo.lock:295-298,4345-4348,4509-4512,4710-4713`). These are dependency identities, not backend/runtime acceptance. Toolchain and lock pins improve repeatability; this session did not establish clean-machine reproducibility or bit-identical outputs.

## Fresh dependency evidence

T171 records an online timeout and an offline exit 101 before compilation. Fresh bounded, credential-disabled observations produced a materially narrower conclusion than “upstream unavailable”:

- **PASS — advertised revision:** anonymous `git ls-remote` returned `2e16697797c8358d16f747a8259354ff2921041a` for both HEAD and `refs/heads/main` at the [primary repository](https://github.com/SK8-ENGINE/skate-3-rust-engine).
- **PASS — pinned manifests:** the [root manifest at that revision](https://raw.githubusercontent.com/SK8-ENGINE/skate-3-rust-engine/2e16697797c8358d16f747a8259354ff2921041a/Cargo.toml) returned HTTP 200; a second bounded retrieval read it. The [pinned skate-data manifest](https://raw.githubusercontent.com/SK8-ENGINE/skate-3-rust-engine/2e16697797c8358d16f747a8259354ff2921041a/crates/skate-data/Cargo.toml) also succeeded and declares package `skate-data`, flate2 **1.1.10**, zstd **0.13.3**, and path dependency `skate-core`. The upstream root pins Bevy **0.18.1**; that alone neither proves nor disproves compatibility of the particular crates consumed here.
- **FAIL — local cached object availability:** read-only `git cat-file -t` could not resolve the pinned commit. `git count-objects -v` reported zero loose objects, zero packs and one garbage temporary pack; no refs were printed. The diagnostic was `fatal: git cat-file: could not get object info`. No cleanup or repair followed.

Owned manifests bind skate-data and related crates to that exact revision (`source/extras/skateboard/engine/skate-disc/Cargo.toml:10-14`, `source/extras/skateboard/engine/skate-sim/Cargo.toml:16-21`); `Cargo.lock:3774-3776` agrees. Remote advertisement plus raw files does **not** prove that Cargo can fetch the complete Git object graph and every transitive dependency.

**Recommendation, not an accepted decision:** a separately authorized recovery lane should retrieve the unchanged pinned dependency, then run the required locked checks once with bounded foreground completion. Preserve diagnostics and distinguish fetch, resolution, compilation and execution. A fresh external cache can isolate the incomplete cache without deleting it. No alternative revision, vendor copy, lock regeneration or dependency removal is justified by this evidence. No library/API replacement is recommended here. Context7 skill was read, but discovery exposed no Context7 tools; library API consultation is therefore **BLOCKED**, not silently replaced with an API compatibility claim.

## Existing lanes and their limits

| Lane | Existing mechanism and current result | Remaining evidence/access |
|---|---|---|
| Formatting | **PASS:** `cargo fmt --all --check` | Not a compilation check |
| Locked Rust checks | Required Clippy all-targets/warnings-denied, workspace tests and build in `CONTRIBUTING.md:61-73`; **NOT RUN** here after cache inspection | Dependency recovery, then actual compiler/test receipts; all-targets is not all-features |
| Native Windows CI | `.github/workflows/ci.yml:1-15` declares `windows-latest`, Rust 1.99.0 and fmt/Clippy/test/build | Workflow exists; current run results **NOT RUN/not inspected**. No smoke, physical devices or source comparison step exists; moving runner/action tags are not a pinned machine image |
| Linux CI | `.github/workflows/progress.yml:19-32` validates reports on Ubuntu, not Rust/runtime portability | Native Linux compilation/runtime lane is absent from these workflows; adding or running one needs separate authorization |
| Progress integrity | **PASS:** nine existing tests via `python -B -m unittest discover -s tools -p test_progress.py -v`; **PASS:** `python -B tools/progress.py --check` | Ledger/report validation only, no gameplay evidence |
| Native runtime | Source-free 180-frame smoke exists | **NOT RUN:** application launch prohibited; actual GPU/audio/controller/launcher operation still required |
| Selected-source comparisons | Source-bound viewers and scripted tours exist | **NOT RUN:** qualified input, controlled reference scenarios, devices/settings and private evidence required for each platform/source combination |

The progress workflow also has a write-enabled refresh job (`.github/workflows/progress.yml:34-60`); source delivery does not authorize triggering or publishing it. The Windows launcher build helper uses locked Cargo builds and copies a product-named executable, but is PowerShell/Windows-oriented (`tools/Build-Launcher.ps1:1-34`); `pwsh` was not found locally.

## Synthetic checks versus runtime qualification

Existing synthetic tests cover relocation/damage/missing files, read-only hashing, safe output boundaries and quarantine (`validation/src/lib.rs:396-503`); chunk verification rejects same-size/same-time mutations and changed executables (`source/data/src/probe_source.rs:454-535`). Controller tests exercise logical edges, deadzones and repeats (`source/visual/src/controller.rs:248-345`), not physical hardware. Save tests cover synthetic round trips and wrong-key rejection (`source/game/save/src/store.rs:154-168`), not a portable cross-source save schema. None of these Rust tests was newly executed or modified here.

The smoke's `LOCALAPPDATA` requirement is evaluated before argument parsing, even before `--log` can override the destination (`source/main/src/bin/smoke.rs:19-40`). It checks development-image loading and timed exit, not sound or gameplay. Linux audio currently returns `no audio output on this platform` (`source/audio/src/lib.rs:247-277`). Launcher font loading, root browsing and viewer naming retain Windows assumptions (`source/main/src/launcher/paint.rs:103-135`, `source/main/src/launcher.rs:299-309,407-416`). A successful dependency fetch cannot resolve these runtime boundaries.

The combat tour records finite damage/Scan/lock checks and explicitly emits `installed_comparison: NOT RUN` (`source/main/src/test_room/enemy_battle_tour.rs:1-7,110-162`). Selected-source session admission remains separate (`source/data/src/probe_source.rs:188-278`); scans retain quarantine (`validation/src/lib.rs:293-324`). T170's historical Windows smoke/tour PASS is not a new native Linux result or a controlled installed/rewrite comparison. Native Windows hardware validation, real Linux GPU/audio/controller checks and qualified Steam/Epic comparisons are distinct outstanding lanes. No cross-build, software renderer, Wine run or report-check PASS can substitute for them.
