# T118 — Launcher executable and game window names

Owner Codex, 2026-10-13. Gate: launcher presentation / P03 tooling; no gameplay gate. Prerequisites: T089 launcher child-process flow, T109 quick start, latest record T117. Starts from `main` at `7ffb955`, package version 0.1.1, pinned Rust 1.99.0. User requests `KH2RW Launcher.exe` for both the executable and launcher window, and `Kingdom Hearts II Re:Written` followed by debug details for the game window.

## Source / implementation

No installed game data is needed for this naming change. Selected-source validation is unchanged; no gameplay session is admitted by this task.

- Set the launcher window title to the exact executable filename.
- Prefix title-screen, room (including talk / field worlds), and event window titles with `Kingdom Hearts II Re:Written`; retain mode, quarantine and controls details.
- `tools/Build-Launcher.ps1` builds the existing Cargo `kh2fm` target and copies its reported executable beside it as `KH2RW Launcher.exe`. Default is release; `-Debug` uses the development profile. Using Cargo's reported artifact path also supports a custom target directory.
- The launcher's existing `current_exe()` flow launches the renamed file for game modes. Viewer binaries still resolve beside the launcher. Update the missing-viewer message and README quick start.

## Evidence / checks

Exact private log locations: `%LOCALAPPDATA%/KH2FinalRewrite/logs/T118-launcher-naming/{fmt,clippy,test,build,release,smoke,artifact}.log`. These contain development diagnostics only and remain outside Git. Local output: `target/release/KH2RW Launcher.exe` (ignored).

- PASS `cargo fmt --all --check`.
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- PASS `cargo test --workspace --locked`.
- PASS `cargo build --workspace --locked`.
- PASS `tools/Build-Launcher.ps1` (release). Initial attempt FAIL: Windows denied removal of the old `kh2fm.exe`; no running launcher was observed and the file was writable. The retry succeeded without changing permissions or stopping any user process. Failure evidence: `release-attempt-01.log` in the same private log directory.
- PASS renamed executable SHA256 equals Cargo output, and the renamed executable's `--version` returns `kh2fm 0.1.1` with exit code zero (`artifact.log`, `version.log`, `version.stderr.log`).
- PASS `git diff --check`; provenance review includes only authored code, build instructions and this record. Executables/logs remain untracked.
- PASS `cargo run -p windows_shell --locked -- --frames 180`: synthetic window opened, sample loaded and clean exit at frame 180 (`smoke.log`, `smoke-runtime.log`). Previous smoke log preserved as `previous-smoke.log` outside the checkout.
- NOT RUN: interactive launcher-to-game title transition; no installed-source session requested for this text change.

## Uncertainty / verdict / next

Cargo's internal development executable remains `kh2fm.exe`; rerun the launcher build script after code changes to refresh the product filename. This is a local build, not a release publication or gameplay fidelity result.

Verdict: PASS for the naming change, release artifact and required development checks. No gameplay or final P03 gate advances. Next: an interactive launcher-to-game handoff check when a selected-source session is next requested.
