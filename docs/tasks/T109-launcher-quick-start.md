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

- A new user has no launch files (`launch.json`, `launch-title.json`, `launch-talk.json` and their configs), so the title screen, test room and Twilight Town tests fail with "launch file is missing"; only the cutscene runs. The guide's step 4 assumes those files exist. Generating them from a validated install is the next task.
- "Walk" on a controller is the stick's analogue reach; the walk threshold is not restated here.

## Verdict / next

PASS for the guide and folder help (local checks). Next: create the test configs for a fresh install so the guide's path works for new players.
