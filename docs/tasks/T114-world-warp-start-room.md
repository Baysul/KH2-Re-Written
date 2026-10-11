# T114 — The World That Never Was as its own world; world and room warp

Owner Claude, 2026-10-11. Gate: development tooling for the field test (no gameplay rule changes). Starts from `main` at 3b7c738. User request (relayed): The World That Never Was showed up mixed into Twilight Town's warp list; make it its own world, and let the F3 debug menu jump to any world and choose the start room, remembered by the launcher. Steam build 15194255 through the session-validated source (`source-manifest-02`).

## Cause

`AreaSource` read only place.bin's `tt` table, keyed by area number alone. After Betwixt and Between's exit (tt40 → area code 100 → eh01, exit-01 §0) the world became `eh`, but the banner, exit guide and the F3 Warp list still used Twilight Town's names. The Warp list then offered Twilight Town names whose rows loaded `eh` areas.

## Changes

- `area.rs`: place names for every world in place.bin, keyed by (world, area) (`place_name`). Each world's installed rooms (`map/<code><nn>.map` present, areas 0..0x40) are listed once at open (`rooms`). `world_name` gives the debug menu's world labels (rewrite labels, not game text).
- `test_room.rs`: the banner, the exit guide and the visited set use (world, area).
- Launcher: a new "The World That Never Was" test (`kh2fm eh`), starting at Alley to Between (eh01, entrance 0) with the same story flags as Twilight Town. `tt_room_config` became `field_room_config(arena, world, saved)` and writes `test-room-<world>.json`. `KH2FM_TT_AREA` still applies to Twilight Town only.
- Saved start: `start-area.json` in the launcher's data folder, `{"tt": [area, entrance], "eh": [...]}`. When a world has a saved room, that world's test starts there. Otherwise it starts at the default room.
- F3 menu, Warp: the first row is "Start here next launch (<code><nn>)". It saves the current room and entrance for this world's launcher test, and its value shows the saved room or "Default". Only Twilight Town and The World That Never Was have launcher tests, so the row is disabled in other worlds. Below it is one row for each world that has installed rooms. Each row shows "Here" or the number of rooms. Choosing a world lists its rooms by place name ("Room nn" when the world has no name for it), each with its code. The list opens on the current room. Enter warps there at entrance 0 through the normal transition, across worlds too. Esc goes back to the world list.

## Checks

PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked` (new: `start_rooms_are_saved_per_world`). PASS `cargo build --release --features skate3` in the main folder after the merge.
PASS sweep: a debug `kh2fm room` started each of eh00–eh29 at entrance 0 for 20 frames with a field config (exit 0, first frame drawn). eh30 is correctly rejected as not installed. Script and logs are private, in the session scratchpad.
NOT RUN: opening F3 by hand and warping between worlds, saving a start and relaunching from the launcher, and starting rooms in worlds other than tt and eh (the owner may be using the PC; no keys were sent).

## Uncertainty

- Other worlds have no story flags set. Their rooms use the loader's fallback (the area's map programs in file order), so their scripts may not match any real story point.
- A cross-world warp uses the same-world iris. Real world changes use a 30-frame black fade (exit-01 §0), which is not ported.
- The world labels are the rewrite's own; "zz" and "es" have no label.

## Verdict / next

PROGRESS: the menu and the launcher entry are built and checked locally. Next: the owner's interactive check of warping and the saved start.
