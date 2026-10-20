# T180 — Bind the current Epic copy as a quarantined source, with saving off

Owner: coding agent. Gate: P02/P03 source-binding and native-platform development evidence only; no source qualification, gameplay, fidelity or compatibility gate advances. Prerequisites: [T172](T172_epic_source_probe.md) (read-only probe of the Epic candidate **E01**: all six `kh2_*` package pairs under `Image/en`; the existing save-table reader rejects its executable), [T174](T174_source_qualification.md) (current Epic copy first; early runs may have saving off with a clear warning; source checks kept), [T176](T176_port_acceptance.md) (checklist A01–A10) and T177–T179. The user asked to continue the port.

## Scope and identity

Let the existing validator, source binding and readers use E01's `Image/en` package folder as a second candidate layout, without loosening any check, and start the title with saving off when the copy's save format is not qualified. Steam behaviour, manifests and save handling stay as they were. No test is added or changed; no game data enters Git.

Baseline `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f` (branch `epic-linux`) plus the uncommitted T177–T179 changes; Rust/Cargo 1.99.0, Bevy 0.19.1. Source: **E01**, the current Epic copy on the x86_64 CachyOS/Wayland host, the same folder T172 inspected. It stays **quarantined/unqualified**; its content identity and file details are private. Host GPU: AMD Radeon RX 6700 XT, RADV, Mesa 26.2.4, Vulkan. No Steam installation of the collection is present on this host, so Steam source runs were not possible here.

## Evidence locations and method

Private evidence bundle **T180-B01**, outside the checkout: source metadata before/after (size, inode, modification and change times of the thirteen selected files and their three folders), the end-to-end run log, launcher logs, screenshots, a copy of the isolated session's launcher folders (settings, manifest, generated configs), check logs and job receipts. GUI runs used a private `kwin-mcp` session with an isolated home, never the user's desktop. The exception is step 10, which the user asked for in their own session; there the agent only started the programs, sent no input and took no screenshots. The source was only read.

## Steps and observations

1. **Candidate layouts** (`validation`). `PACKAGE_FOLDERS` lists `Image/dt` (Steam) and `Image/en` (E01). New helpers: `required_files_in(folder)`, `index_files(folder)`, `package_folder_of(files)` (a manifest's folder, only when its files are exactly the executable and one folder's twelve packages, each once) and `detect_package_folder(root)`. A scan uses the one folder holding package files; files in both folders are refused as ambiguous, so nothing is mixed or chosen silently. `required_files()` still means the Steam set. The manifest schema is unchanged, so existing Steam manifests still bind. A non-Steam layout gets a neutral product name, edition note and an extra unknown (language routes, package precedence and executable-derived tables unqualified). Messages no longer assume Steam.
2. **Binding** (`data::probe_source`). `ProbeSource` takes the folder from the manifest's file set and refuses a manifest that matches neither layout. Full and chunk verification and `open_index` use only the bound folder's files. New accessors: `package_folder()`, `indexes()`, `candidate_files()`. `Library::open` uses the bound indexes; the duplicate Steam-only `path_lookup::INDEXES` list, now unused, is removed.
3. **Readers and tools.** The image and model inspectors and the plain, original and message probes check selections against the bound indexes. The `data` inventory command detects the folder. In `main`, the launcher's fast manifest check uses the manifest's folder; the test room's help callouts, item pictures (the fifth package, as observed in Steam; another layout's fifth package must also hold them, or the read fails visibly), board-model search and settings-room language swap use the bound indexes.
4. **Saving off** (`save_files`, `title`). `FileBackend::open_or_off` turns saving off, with a reason, when the bound copy is not in the Steam layout (its save format and account rules are unqualified, whatever its executable's table reads as) or when its executable yields no usable save table. Other errors still fail. With saving off, no save file is read or written, the slots are empty, and saving or deleting returns failure, so the menu shows its existing write-failure notice instead of reporting success. The title prints a warning and its window title ends in "SAVING OFF". Unattended runs keep their generated or supplied in-memory saves. For E01 the old reader still returns `TableNotAPermutation`; the first runs below showed that as the reason, before the layout rule was added. A final title run (step 9) shows the layout reason.
5. **Launcher text.** The quick-start guide, the wrong-folder message and the developer-config version message now cover Epic copies.
6. **End-to-end run.** In the isolated session, the existing ignored test `every_test_starts_from_an_empty_data_folder` ran against E01 from an empty data folder (`KH2_TEST_FOLDER` set to E01; temporary files inside T180-B01). It scanned and hashed the thirteen files and wrote a manifest. It then generated the test configs by name and ran each program from the built binary. The title showed the saving-off warning and reached "Title ready". The test room (three runs: arena, Twilight Town, The World That Never Was) decoded in 6.8–8.5 s with its first frame after 7.5–9.1 s; the event player loaded event 613 (5 actors, 156 commands). **1 passed, 0 failed** in 66.5 s, with no Vulkan validation, error or panic lines.
7. **Launcher GUI run.** The launcher found E01 through its settings file, which held the folder and no manifest, inside the isolated home. It hashed the folder ("Reading the game files once", 15.3 GB) and opened the test list. Starting the title from the list generated the configs, showed "Loading Title screen", then the Disney and Square Enix logos and the full title menu (New Game, Load, Back) with "SAVING OFF" in the window title. Left idle, the title handed off to its demo, which is not built yet, and returned to the launcher with success. This is existing behaviour, not a port defect. Esc quit the launcher. No validation, error or panic lines.
8. **Source untouched.** Source metadata was identical after both runs; nothing in the source tree was newer than the run start. The isolated home held only `launcher.json`, the manifest and the generated configs; there was no save file or save folder anywhere in it.

9. **Final title run.** After the layout rule and the full checks, the title ran from E01 for 240 frames in a fresh isolated session with the saved manifest and config: exit 0, warning "saving is off for this copy (the save format of a copy with an Image/en folder is not qualified)", no validation, error or panic lines. Source metadata was still unchanged, and no rewrite save folder exists on the host.

10. **User-session run.** At the user's request, the title (twice, directly) and then the launcher ran in the user's own Wayland session, with sound through its PipeWire default device. A launcher settings file held E01's folder, and the launcher scanned it into its manifest. The title reported `audio output: default, I16, 48000 Hz, 2 channels`. The user reported: sound worked with no crackling, and controller input worked. Maximizing the test-room window froze the party on screen while combat continued; that defect is fixed in [T181](T181_scene_resize_freeze.md). Esc in the title quits it, or returns to the launcher when the launcher started it. This differs from the PC original, where Esc opens the game settings; the title module documents it, and it is unchanged here. Afterwards there was still no rewrite save folder on the host. The run created `KH2FinalRewrite` folders under the user's config and data directories (launcher settings, manifest, generated configs).

Two side observations from step 7:

- **Missing underscores in small launcher text.** The footer's 16 px folder path showed "KH 1.5 2.5" for a name with underscores; the 18 px path on the checking screen showed them. Pixel rows below the footer's baseline are empty. Likely cause: Liberation Sans Bold's underscore is under one raster pixel thick at that size (`RASTER` 1.5), and its quad can miss every pixel centre. This is cosmetic, comes from T178's launcher fonts and was not fixed here.
- **Page Down not observed.** In the folder list, Page Down from the test tool did not move the cursor, though Down and Enter did. The launcher code handles Page Down on every list, and the tool sends evdev code 109. Which side drops it was not established.

To reach E01 in the GUI, the run used the settings file instead of browsing 66 rows by key. The runner's loop guard blocks repeated identical input calls.

## Checks

| Check | Result |
| --- | --- |
| E01 end-to-end ignored test (scan, configs, title, three test rooms, event load) on Linux/Wayland | PASS: 1 passed, 0 failed; zero validation/error lines |
| Launcher binds E01 by full scan; test list; title started from the launcher | PASS (functional, isolated session) |
| Saving off on E01: warning, SAVING OFF title, no save file read or written | PASS for the warning, title and absence of files. The save and delete refusal paths were not exercised in the GUI |
| Final saving-off reason text after the layout rule | PASS: layout reason shown; exit 0 after 240 frames |
| Source metadata unchanged (13 files, 3 folders) | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS (first attempt FAIL: one needless borrow in `test_room.rs`, fixed and rerun) |
| `cargo test --workspace --locked` | PASS: 725 passed, 0 failed, 1 ignored, 28 test programs |
| `cargo build --workspace --locked` | PASS |
| Windows-target clippy of `main`, `visual`, `audio`, `data`, `validation`, all targets, warnings denied | PASS (compile-only cross-check; `CARGO_FEATURE_PURE=1` for `blake3`, as in T178) |
| Steam layout on real Steam data (S01) | NOT RUN: no Steam installation of the collection on this host. Existing synthetic Steam-layout unit tests run in the workspace tests |
| Ambiguous-layout refusal and other negative source probes (A03) | NOT RUN on real data; covered by code review only |
| Game sound from E01 on Linux | PASS (user-reported, step 10): title and test room, no crackling heard. The isolated session has no sound server ("Host is down") |
| Controller input from E01 on Linux | PASS (user-reported, step 10) |
| Window resize or maximize in the test room | FAIL in step 10 (scene froze); fixed and verified in [T181](T181_scene_resize_freeze.md) |
| E01 qualification, language/package-precedence rules, controlled comparisons (A04, A09) | NOT RUN: out of scope; E01 stays quarantined |
| Native Windows regression | NOT RUN: no Windows host |

Checklist (T176): this slice adds functional evidence for **A02** (the launcher binds a real source and starts a helper program on Linux) and partial evidence for **A08**: the warning is shown and no save is touched, but the persistence actions were not attempted and E01 is not a qualified source. A03, A04, A07 and A09 do not pass.

## Uncertainty, verdict and next work

**PASS** for this slice: E01 binds as a quarantined candidate through the real validator and readers, and the current programs start from it on Linux with saving off and a warning. No gate advances, E01 is not qualified, and the NOT RUN rows above stay open.

The required checks ran with a private build folder, because an editor's rust-analyzer also builds in the checkout's `target/`.

Limits: package precedence between E01's packages follows the existing "first package holding the name" rule, an inference T172 already flagged. The item-picture index assumes the fifth package. Language selection in the settings room only swaps among the languages the bound indexes hold. The no-save rule is deliberately broad: any non-Steam layout has saving off until a portable save format is decided (deferred ticket 07). Useful regression tests, each needing approval first: `detect_package_folder` (none, `Image/dt` only, `Image/en` only, both → refused); `package_folder_of` (exact set, a duplicate, a mixed set → none); and a saving-off `FileBackend` whose save and delete fail and create no file.

Next, pick one: Epic install detection (Heroic, Legendary or the Epic launcher's records) so browsing is not needed; the small-text underscore fix; the Page Down question; Esc in the title opening the game settings as on PC; or the deferred save-format work.
