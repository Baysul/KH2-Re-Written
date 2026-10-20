# T177 — Start the native Linux build and source-free window

Owner: coding agent. Gate: P03/native-platform development evidence only; no final source, gameplay, fidelity or compatibility gate advances. Prerequisites: fidelity contract, milestones, unknowns, folder guide, glossary, T176 and the approved source/platform/first-port acceptance decisions. The user instructed implementation to begin after the planning handoff.

## Scope and identity

Start with the unchanged pinned dependency, capture real Linux compiler diagnostics, fix the first native build/startup boundaries and verify the existing source-free smoke. This is an implementation slice, not completion of the Linux/Epic acceptance matrix. No new or changed tests are authorized.

Rewrite baseline: `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`, branch `epic-linux`, Rust/Cargo 1.99.0, Bevy 0.19.1. The locked upstream skateboard dependency remains revision `2e16697797c8358d16f747a8259354ff2921041a`; no member, revision or existing package version is deliberately replaced. Optional Linux backend dependencies, if needed, must be explicit and reviewed without a general lockfile refresh. Existing uncommitted planning work is preserved.

Host scope: current x86_64 native Linux/Wayland development host only. Steam candidate S01 and Epic candidate E01 remain unqualified and are not used by this source-free slice. No installed game, vendor executable, source-derived data, save, import or gameplay session is opened. No Wine/Proton, installation writer, publication or release is involved.

## Evidence locations and method

Private evidence bundle **T177-B01**, outside the checkout under the private research state directory. Its unique directory is recorded in the local tool receipt. Exact artifacts: `baseline-build.log`, `recovered-baseline-build.log`, `baseline-receipt.json`, `workspace-metadata.json`, `lock-before.sha256`, `Cargo.lock.before`, `wayland-check.log`/`wayland-check-receipt.json`, `smoke-*.log`, `final-{fmt,clippy,tests,build}.log`, `final-verification.json`, the private smoke logs, and the isolated-session logs/captures under `gui/` and `gui-input/`. These artifacts contain machine-specific paths and stay private. Public records contain only opaque evidence IDs and sanitized findings.

Long-running commands use supervised `jev-fabric` jobs with finite deadlines and terminal receipts. Waiting uses completion signals, not fixed sleeps. GUI checks, if reached, use a private isolated KWin session, never the user's desktop; captures remain private and the session must be stopped on every exit. Software rendering is not hardware qualification.

## Steps and observations

1. Re-read the required records and approved handoff, inspect the native toolchain/workspace, and preserve the source/save boundary.
2. Run `cargo build --workspace --locked --offline -j 2`. FAIL before compilation: the pinned `skate-data` Git dependency is missing from the local offline cache. The original cache is not deleted or repaired in place.
3. Create a separate private Cargo home, reuse the registry cache, and fetch/build with the unchanged revision using `cargo build --workspace --locked -j 2`. Fetch succeeds; the terminal receipt exits 101 at the first compiler blocker: `The platform you're compiling for is not supported by winit` in pinned winit 0.30.13. This is a backend-feature error, not an unsupported native CPU/OS claim.
4. Inspect the pinned Bevy manifest and winit platform guard before editing. Bevy 0.19.1 exposes `wayland = ["bevy_internal/wayland"]`; both owned windowing packages disable defaults but select no Linux backend. The literal upstream guard and reproduced compiler error justify a focused feature fix. Context7 returned no matching version-specific documentation; the locally fetched exact-version sources are the authority.
5. Add an explicit Linux-only `wayland` dependency feature to `source/main/Cargo.toml` and `source/visual/Cargo.toml`, preserving their existing disabled defaults and Windows declarations. An initial locked attempt correctly refuses the additional backend packages; perform one minimal lock update through `cargo check --workspace -j 2`, then resume locked commands. All 444 original package/version/source/checksum entries remain unchanged; 45 entries are added for the backend/conditional dependency graph. No general dependency refresh is performed.
6. The workspace check now PASSes (exit 0), including the existing optional workspace members. This removes the reproduced winit compiler failure. Launcher/audio/source support is not inferred from type-checking.
7. Run the linked smoke in a private isolated KWin Wayland session. It panics before parsing arguments: `LOCALAPPDATA is required on Windows`, even with `--log`. Fix `source/main/src/bin/smoke.rs` so `--log` is parsed first. Only when it is absent does Linux use an absolute `$XDG_STATE_HOME` or `$HOME/.local/state`, plus `KH2FinalRewrite/logs/smoke.log`. Relative XDG values are ignored and a missing home fails with an explicit message. The Windows default path and message are unchanged.
8. The window then renders the generated sample on the real GPU (Vulkan, RADV NAVI22 discrete adapter, Mesa 26.2.4) and exits 0, but two runs each log eight Vulkan validation errors at startup: a presented swapchain image left in `UNDEFINED` layout and an acquire semaphore already signaled. This is not counted as a clean pass. Pinned Bevy 0.19.1 source shows two causes. The upscaling node returns without drawing while its pipeline is pending, but the view is still marked for presentation. On Wayland, the first frame must present before any camera has drawn.
9. Linux-only synchronous pipeline compilation reduces the errors from eight to two. A Linux-only render system clears **only** a window's initial Wayland buffer (with the existing clear colour) before `render_system` presents it. That removes the rest. Validation is not disabled, later frames are not changed, and no dependency is altered. Both fixes are limited to the source-free smoke; the launcher and game windows need the same check when they are ported.
10. Verify the final binary in fresh isolated sessions (results below). Then correct one observation error: a first input check was wrongly logged as BLOCKED. After a long held key, the sample had simply moved out of view, and an unrelated KCalc capture was unchanged, not black. A repeat with a brief held `a` moved the sample 4 px left. Escape still exits cleanly.

## Checks

| Check | Result |
| --- | --- |
| Unchanged pinned Git dependency available in separate cache | PASS: locked resolution reaches native compilation; original cache retained |
| Unmodified Linux baseline build | FAIL: known missing Linux winit backend; compiler reached, no source admission implied |
| Native workspace type-check after feature fix | PASS: `cargo check --workspace -j 2`, exit 0; exact original lock entries retained |
| Final `cargo fmt --all --check` | PASS |
| Final `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| Final `cargo test --workspace --locked` | PASS: 725 passed, 0 failed, 1 already-ignored test (needs an installed game) across 28 harnesses; no test added, changed, skipped or filtered |
| Final `cargo build --workspace --locked` | PASS |
| `cargo run -p main --locked -- --frames 180` in an isolated Wayland session | PASS: exit 0, `clean_exit; frames=180; sample_loaded=true`, zero validation/error lines |
| Same smoke with a fresh empty GPU shader cache | PASS: exit 0, zero validation/error lines |
| Smoke log location | PASS: `--log` works with no home/XDG variables; absolute XDG state used; relative XDG ignored (HOME fallback, no relative folder created); missing home without `--log` fails clearly (exit 101, intended) |
| Virtual keyboard input and Escape exit | PASS in isolated KWin (EIS input): held `a` moves the sample left; Escape exits 0 |
| Real GPU rendering | PASS for this smoke only: discrete AMD adapter via Vulkan/RADV, not software rendering. Presented to a virtual KWin output |
| Physical display presentation, scaling, focus, frame pacing; physical keyboard/controller | NOT RUN |
| Linux launcher (`KH2RW`), viewers, fonts, helper programs, Linux storage paths | NOT RUN: not yet ported |
| Linux sound output | NOT RUN: Linux still reports no audio output |
| Windows native build/runtime regression | NOT RUN: no Windows host here. Windows code paths and manifests are unchanged except the shared smoke structure, which keeps its Windows default path |
| Steam or Epic source qualification; gameplay; controlled comparisons | NOT RUN: source-free slice; both candidates stay quarantined |
| Documentation/provenance and existing progress checks | PASS: see final record checks |

## Uncertainty, verdict and next work

**PASS for this slice: native Linux build plus the source-free Wayland smoke window.** The required locked checks pass on Linux, and the smoke renders on the real GPU without validation errors. This covers checklist item A01 on the Linux host only. It does not cover the launcher, sound, Steam/Epic sources, gameplay, Windows regression or physical-device lanes, which stay NOT RUN. No coverage unit or final gate changes.

Uncertainty: the startup-validation workaround lives in the smoke binary. Other Bevy apps (launcher, title, test room, viewers) may show the same Wayland first-frame problem and must each be checked with validation on. The Windows lane was not exercised. The lock gained 45 entries for the Wayland backend and Bevy's conditional clipboard graph, with all original entries unchanged; Windows resolves them only where their target conditions apply. No regression test was added: a useful one would run the smoke in a headless Wayland compositor and fail on any Vulkan validation line, and needs approval first.

Later note: [T178](T178_linux_launcher_boundary.md) replaced the two smoke-only fixes with a shared Linux window-clear plugin used by every Bevy window. Synchronous pipeline compilation turned out to be unnecessary and was removed.

Next: **T178**: port the launcher's host boundary (Linux storage locations, bundled licensed launcher font, native folder browsing and helper launching), checking each Bevy window for the same startup validation errors. Then Linux sound, then the Epic source profile and the warned no-save startup path.
