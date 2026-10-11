# T109 — Launcher quick start guide and folder help

Owner Claude, 2026-10-11. Gate: launcher usability (no gameplay gate). Starts from `main` at 46ad284. User request (relayed): a short quick start guide for people new to the repo, the common path made obvious, clear errors when the game folder is wrong.

## Prerequisites / source

No game data is read beyond what the launcher already checks. The controls listed come from the code: `windows_shell::input::read_devices` (key and button map) and the test room's `read_pad` / `camera_control` (which logical bits act).

## Implementation

- `launcher.rs`: a `Guide` screen with four steps and a controls table [launcher text, not the original's]. It opens on the first launch (`guide_seen` in the launcher settings), and F1 or the controller's Back button reopens it from the folder and test lists. Continue goes to the test list, the remembered folder, the one Steam install found, or the folder list, in that order.
- `install.rs`: `find_game_below` uses the game folder up to three levels below a picked folder (a Steam library or `common` chosen by mistake) when exactly one is there. The "no executable" error now names the Steam folder and says how to find it (Manage > Browse local files).
- README controls table brought in line with the bindings above.

## Checks

PASS fmt; PASS clippy (warnings denied, workspace); PASS workspace tests (includes the new `game_found_below_a_library_folder`); PASS workspace build; PASS release `kh2fm` build with `--features skate3`. NOT RUN: a full first-run playthrough on a clean machine. Screenshots of the guide (fresh data folder) and of the test list were checked by eye; they are private and not committed.

## Uncertainty

- Superseded by the follow-up below: a new user had no launch files, so only the cutscene ran.
- "Walk" on a controller is the stick's analogue reach; the walk threshold is not restated here.

## Follow-up: test configs for a fresh install

A new player has none of the private launch files (`launch.json`, `launch-title.json`, `launch-talk.json`) or the configs they name. Every installed file the owner's working configs (`title-01`, `test-room-arena-07`, `test-room-talk-03` and the four model selections) point at was matched to its path name by MD5 against the HED keys (for example `obj/P_EX100.mset`, `menu/us/pause.2ld`, `field2d/us/zz0command.2dd`).

- `launcher/test_configs.rs`: those configs (a JSON string) with each file written as `{"$file": "<path name>"}` and their test-room values unchanged. It holds names and test-room numbers only, no game bytes.
- `install::test_args`: a developer's own launch file for this build still wins. Without one, `made_config` binds the folder's manifest (`ProbeSource::bind`), opens the six checked indexes (`named::Library`), fills in index, ordinal and digest for each name (the first package holding it), and writes `auto-*.json` with the build's identity into the launcher's data folder. Title screen, Test room (room and talk combined) and Twilight Town use them; each program still admits the source itself when it starts.
- Check: on the installed build, the made configs equal the owner's working configs field for field, except the selection file names. Before the first-package rule, `se/se000.win32.scd` resolved to the sixth package's copy; it now matches. The combined play config equals the owner's `test-room-combined.json`.
- NOT RUN: an interactive start of each test from an empty data folder (the configs are equal to ones that run).

## Verdict / next

PASS for the guide, folder help and made configs (local checks). Next: an interactive first-run check from an empty data folder.
