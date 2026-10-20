# T178 — Port the launcher's host boundary to Linux

Owner: coding agent. Gate: P03/native-platform development evidence only; no source, gameplay, fidelity or compatibility gate advances. Prerequisites: T177 (native Linux build and source-free smoke), the approved [native Linux platform decision](T175_native_linux_platform.md) and the [first-port acceptance checklist](T176_port_acceptance.md). The user asked to continue the implementation.

## Scope and identity

Make the launcher (`KH2RW`) usable natively on the Linux/Wayland host: Linux storage locations, launcher fonts, folder browsing, helper launching and Linux Steam library detection. Also replace T177's smoke-only Vulkan startup workaround with one shared plugin used by every Bevy window. Windows behaviour is kept: each Windows path is unchanged code under `cfg`. No test is added or changed. No game source is opened.

Rewrite baseline `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f` (branch `epic-linux`) plus the uncommitted T177 changes; Rust/Cargo 1.99.0, Bevy 0.19.1, lockfile unchanged from T177 (no new dependency). Host: the x86_64 CachyOS/Wayland development machine only. This machine has a Steam installation but **no** copy of the collection (app 2552430), so the Linux × Steam cell cannot be exercised here. Steam candidate S01 and Epic candidate E01 remain quarantined and unused.

## Evidence locations and method

Private evidence bundles **T178-B01** (logs, receipts, isolated-session captures, private XDG folders) and **T178-F01** (the downloaded font release and its extraction), both outside the checkout under the private research state directory. Public records carry only opaque IDs and sanitized results. Long commands ran as supervised `jev-fabric` jobs with terminal receipts. GUI checks used a private isolated KWin session with an isolated home, never the user's desktop. They rendered on the real GPU (Vulkan, RADV NAVI22 discrete adapter, Mesa 26.2.4) to a virtual output; the session was stopped afterwards.

## Steps and observations

1. Inventory the Windows-only host assumptions in owned code (outside the optional skateboard extra): `LOCALAPPDATA` data/log folders, launch-file fallbacks in title and test room, Windows Steam roots, drive-letter browsing, `.exe` helper names with lossy path text, Windows font files, and the vendor-save folder lookup (`USERPROFILE`, left for the later save/no-save work).
2. **Shared window clear.** Add `visual::window_clear::WindowClearPlugin`. On Linux it clears each window frame to the clear colour before the render graph runs; elsewhere it adds nothing. With it, T177's synchronous pipeline compilation is no longer needed: the smoke with a freshly created empty GPU shader cache and a warm repeat both exit 0 with zero validation/error lines. So the game keeps the same asynchronous pipeline behaviour as on Windows. The plugin replaces the smoke's private system and is added to all eight Bevy windows (smoke, launcher, title, test room, event player, image/model inspectors, room preview).
3. **Storage.** `launcher/install.rs` keeps its Windows `data_dir()` unchanged. On Linux, private files are split per the approved XDG boundary. Settings (`launcher.json`) go to `$XDG_CONFIG_HOME/KH2FinalRewrite`. Manifests, generated configs, launch files and saved start rooms go to `$XDG_DATA_HOME/KH2FinalRewrite`. Viewer logs go to `$XDG_STATE_HOME/KH2FinalRewrite/logs`. Absolute XDG values are used, otherwise the `$HOME` defaults; relative values are ignored, and a missing home fails with a clear message. There is no checkout or working-directory fallback and no migration. On Windows `settings_dir()` is the data folder, so nothing moves. Title and test room share one launch-file lookup: the Windows candidates are unchanged, Linux uses the data folder.
4. **Steam libraries.** On Linux, look in `$XDG_DATA_HOME/Steam` (default `~/.local/share/Steam`), `~/.steam/steam` and the Flatpak root, then the libraries listed in each `libraryfolders.vdf`; install folders are canonicalized, so the symlinked roots collapse. Detection only offers folders; validation still decides.
5. **Browsing and helpers.** Folder browsing (and the skateboard ISO browser's start list) begins at the drive letters on Windows (same as before), and at the home folder and `/` elsewhere. The root heading reads "Choose where to look" off Windows. Viewers are found with the platform executable suffix and get their path arguments as OS strings, not lossy text. The missing-viewer message names the right launcher file per platform; on Windows its text is unchanged.
6. **Fonts.** Off Windows the launcher embeds unmodified Liberation Sans 2.1.5 Bold Italic (headings) and Bold (body) from the upstream release tarball (SIL OFL 1.1; hashes and tag in `source/main/assets/fonts/README.md`). The licence is in `docs/release/liberation-fonts-LICENSE.txt` with trailing whitespace normalized. The distro's own copies differ byte-for-byte (rebuilt from source), so they are not the provenance. Windows still reads its installed fonts and its error text is unchanged. Allowlist entries cover exactly the two fonts and their README; provenance is recorded in `docs/release/provenance.md`.
7. **Windows cross-check.** `cargo clippy -p main -p visual -p audio --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings` passes with zero warnings, type-checking every touched `cfg(windows)` branch. One probe setting: the first attempt stopped because `blake3`'s build script wanted Microsoft's `ml64.exe`. Setting `CARGO_FEATURE_PURE=1` in that one command's environment selects its pure-Rust path; no project file or feature changed. This is a compile check, not native Windows evidence.
8. **Launcher on Wayland.** In the isolated session with private XDG folders: the quick start guide renders with the bundled fonts; Continue reaches "Select the game folder" (no Steam install offered, correctly); Browse lists the home folder and `/`. Browsing `/` lists folders with "Use this folder" disabled (no game there), Esc returns to the root list, and Quit exits 0. Zero validation, error or warning lines. `launcher.json` was written only under the private XDG config folder; nothing appeared under the real home's config/data folders.
9. **Required checks** (results below). The first full `cargo test` run failed at compile time in the `visual` doc-test step, before any doc test ran: `rustdoc` reported `can't find crate for bevy` although the library file existed. All 725 unit tests had passed. A solo `cargo test -p visual --doc` then passed, and a full rerun passed with no source change. The cause is unconfirmed, most likely a stale library from the Windows-target check that ran just before with its probe setting; the failure is kept here rather than dropped. Later note (T179): the same failure recurred when an editor's rust-analyzer rebuilt `bevy` in the shared `target/` folder during a test run, which is the more likely cause here too; later checks use a private build folder.
10. A second launcher run with the rebuilt binary opens straight on "Select the game folder": the saved settings carried over. The browse heading reads "Choose where to look", and the window's close button exits 0 with zero validation/error/warning lines. Across both runs the only file written in the private XDG folders was the launcher settings file.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` (Linux) | PASS |
| Windows-target clippy of `main`/`visual`/`audio`, all targets, warnings denied | PASS (compile-only cross-check; `CARGO_FEATURE_PURE=1` probe setting for `blake3`) |
| `cargo test --workspace --locked`, first run | FAIL: `visual` doc-test build could not load `bevy`; 725 unit tests passed, 0 failed, 1 already-ignored |
| `cargo test -p visual --doc --locked` (solo) | PASS |
| `cargo test --workspace --locked`, rerun | PASS: 725 passed, 0 failed, 1 already-ignored, 28 harness results, no source change |
| `cargo build --workspace --locked` | PASS |
| Smoke, cold and warm GPU cache, with the shared plugin and no synchronous compilation | PASS: exit 0 both, zero validation/error lines |
| Launcher start, guide, folder screen, browsing, Esc, Quit (isolated Wayland, real GPU) | PASS: exit 0, zero validation/error/warning lines |
| Launcher settings persist between runs; window close button | PASS: second run skips the guide; close exits 0, zero validation/error/warning lines |
| Linux storage locations | PASS for settings (written only under the private `$XDG_CONFIG_HOME`); data/state paths reached only by later flows |
| Linux Steam library detection | NOT RUN against a real library: no collection install on this host |
| Title, test room, event player, viewers on Linux (and their window clear) | NOT RUN: each needs a validated game source first |
| Launcher test launch, drag-and-drop, viewer launch | NOT RUN: need a bound source |
| Physical display/keyboard/controller; Linux sound | NOT RUN |
| Native Windows runtime | NOT RUN: no Windows host |
| Source qualification, gameplay, controlled comparisons | NOT RUN: both candidates quarantined |

## Uncertainty, verdict and next work

**PASS for this slice: the launcher's Linux host boundary.** The launcher runs natively on Wayland with bundled fonts, XDG folders, native browsing and validation-clean frames. Required checks pass on Linux, and the touched Windows code type-checks for the Windows target. This adds checklist item A02 for the launcher screens that need no source. Source-bound launcher flows, the other windows on Linux, Steam detection against a real Linux library, sound, physical devices and native Windows stay NOT RUN. No coverage unit or final gate changes.

Limits: the launcher was exercised only on screens that need no game source. Liberation Sans metrics differ from Segoe UI/Trebuchet: the screens seen fit, but long texts on the test list, check and error screens were not visible here. `test_args` still turns test-launch paths into lossy text: changing its return type would mean editing an existing test, which needs approval. That only matters for non-UTF-8 game folder paths. The title screen's save backend still reads the source executable's save table and looks for vendor saves under `USERPROFILE`. That belongs to the Epic/no-save slice. No regression test was added. Useful ones would check `xdg_dir` (absolute, relative and missing values) and run the launcher in a headless Wayland compositor, failing on any Vulkan validation line; both need approval first.

Next: **T179**: a Linux sound output adapter behind the existing mixer. Then the Epic source profile with the warned no-save title start, which also unlocks Linux checks of the title, test room and viewers.
