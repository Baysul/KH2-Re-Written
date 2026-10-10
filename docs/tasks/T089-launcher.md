# T089 — Launcher window: game folder, build check, test menu

Owner Claude, 2026-10-10. Gate P22 (front-end scope, developer tooling). Starts from T086–T088 (boot splash, title edits, journal). User request: opening `kh2fm.exe` shows a KH2-styled menu that asks for the game folder, loads it, then asks how to test the game; the title test includes the Disney/Square Enix sequence; the NPC and battle rooms are merged into one test; fast and smooth.

The launcher is a **rebuild addition**, not an original screen. Its look is our own design in a KH2-like style.

## Prerequisites / source

Same selected source as T086–T088: Steam app 2552430, build 15194255, content identity `a9ca1e03…`, quarantined. The private manifest is `.local/test-room/source-manifest-02.json`. Configs and launch files are private, in `.local/test-room/`.

## Implementation

- **`kh2fm` with no arguments** opens the launcher window (`bin/kh2fm/launcher.rs`) instead of the console menu. `kh2fm title|room|talk [args]` are unchanged. `kh2fm play` is new: the combined room.
- **Windows subsystem.** `kh2fm.exe` no longer opens a console window behind it. Output still reaches pipes and redirects; the launcher reads its tests' output that way.
- **Drawing without game files** (`launcher/paint.rs`):
  - the look is deliberately plain, at the owner's request: a dark screen, one heading and one list, with a blue bar on the selected row;
  - text is rasterised from Windows' own fonts at runtime (Trebuchet MS Bold Italic for headings, Segoe UI Semibold for rows) with `fontdue`;
  - everything is drawn through `ui2d`.
  - No font or art is committed.
  - The first version (glows, sparkles, side panel) was replaced after the owner said it looked too busy.
- **Finding the folder** (`launcher/install.rs`):
  - Steam's default libraries and `libraryfolders.vdf` are searched for `appmanifest_2552430.acf`; installs holding the KH2FM executable are offered.
  - The user can also browse drives and folders inside the launcher, or drop the folder (or the exe) onto the window.
- **Check.**
  - *Quick check:* an existing manifest for the folder must be self-consistent, and every required file must have the size and modified time it was hashed at. No file is read.
  - *Full scan:* when no manifest matches (new folder, or Steam changed files), the folder is hashed in full with a progress bar. The new manifest is written to `.local/test-room/source-manifest-NN.json`. `source_identity::scan_with_progress` and `required_bytes` were added for the progress bar.
  - *Build check:* the folder's content identity must equal the identity the test configs were made for (the title config's). Any other build is refused and stays quarantined.
- **Memory.** The folder and its manifest are saved to `.local/test-room/launcher.json`. Later launches quick-check it and open straight on the test menu. "Change Game Folder" (F2 / Y) goes back to the picker.
- **This check does not replace admission.** Every test still binds the source and runs `verify_session` itself when it starts (unchanged).
- **Tests.**
  - *Title Screen* runs `kh2fm title` with the full T086/T087 boot (Disney, Square Enix, GLURBUS STUDIOS, copyright, intro, Final Re:WRITE).
  - *Test Room* runs `kh2fm room` on a combined config. The combined config is the arena config from `launch.json` plus the `npc` block of the talk config from `launch-talk.json`. Axel is moved aside to offset (-450, 0, 650), yaw 2.5 [launcher choice]. It is rebuilt on every launch as `test-room-combined.json` beside the arena config, so it follows both configs as the test-room thread changes them. No test-room code changed.
- **Running a test.**
  - The test runs as a child `kh2fm` process with the chosen folder and manifest.
  - The launcher shows NOW LOADING until the child logs its window creation, then hides.
  - When the test closes, the launcher comes back on the test menu.
  - If the test fails, the launcher shows the test's own error lines.
- **Input:** keyboard (arrows/WASD, Enter/Space, Esc/Backspace, PgUp/PgDn, F2), mouse (hover, click, wheel, right-click back) and gamepad (D-pad/stick, A, B, Y, bumpers).

## Evidence (private, outside Git)

- **Combined room by itself:** `kh2fm room <root> source-manifest-02.json test-room-combined.json --autoplay` (debug build) loaded in about 25 s.
  - Captures in the session scratchpad (`cap1/test-room-00.png`, `-04.png`) show Sora, Axel to the side and the Shadow emerging, with the HUD.
- **Launcher on the install** (debug build):
  1. The picker listed the Steam install.
  2. Choosing it quick-checked against `source-manifest-02.json` with no visible wait, saved `launcher.json`, and opened the test menu (build 15194255 in the footer).
  3. Title Screen started the child. Its window opened about 1 s after the press and played the boot/intro.
  4. Esc in the title returned to the launcher's test menu.
  5. Test Room's window opened after about 13 s.
- Window captures (`l1`–`l8`, `c1`–`c4` in the session scratchpad) were made with a screen copy. One of them (`c4`) caught another application over the window, so the combined room's launcher run is confirmed by its window title only.

## Checks

- PASS `cargo fmt --all --check`.
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- PASS `cargo test --workspace --locked`. New synthetic tests:
  - scan, then quick check, then a changed file needing a rescan;
  - a folder without the exe is rejected;
  - settings round trip;
  - VDF library paths;
  - combined config adds the NPC and checks the build identity;
  - design-space mapping;
  - scan progress counts every byte.
- PASS `cargo build --workspace --locked`.
- NOT RUN: the windowed smoke (`cargo run -p windows_shell --locked -- --frames 180`). The owner was using the machine.
- NOT RUN: the full-scan path on the real install (it reads about 16 GB; the existing manifest matched). It is covered only by the synthetic test.
- NOT RUN: gamepad and drag-and-drop on the real window.

## Uncertainty

- The window is hidden when the child logs bevy's "Creating new window" line. A test that fails after that line comes back through the exit path, but a test that never logs it keeps the launcher on NOW LOADING until it exits.
- Axel's spot and facing in the combined room are a launcher choice, not reference data.
- The launcher's art and fonts are not the original's menus.

## Verdict

PASS for the launcher flow on the installed build, within the checks above. No gameplay gate advances.

## Next

- If the owner wants it, New Game on the title could hand off to the combined room.
- Once a folder is bound, the test menu could draw with the game's own menu art.
