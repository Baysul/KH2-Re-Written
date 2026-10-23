# T214 — World map: the pause menu, the journal, per-world play time and the Gummi hand-off

Owner Claude, 2026-10-23, requested by burba in the project chat ("let's work on the map system
please recreate that 1:1 using ghidra dont stop until its perfect"); second round handed over by
the project's coordinator session after T212. Gate: development of the world map program only;
no final gameplay acceptance is inferred. Prerequisites: T212 (the world map's cursor, lock and
fades), T088 / T093 / T164 (the camp and the journal in the field program). Build identity:
branch `claude/project-thread-ofpsmp` from `main` at `cb8a139`, merged with `main` at `134195f`,
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each
run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-1to1-20261023/`
(plain headless Ghidra 12.1.4, read-only on the analysed world map project; decompiles
`out/d14`, `out/d15`, `out/d19`..`out/d21`, disassembly `out/dis2`, references
`out/refs6.txt`..`out/refs8.txt`, data `out/dd6.txt`, `out/dd7.txt`). Unattended runs and captures
in the private `kh2-worldmap-out/` (`r2-*`).

## What the executable does

- START opens the camp as in the field: `FUN_1401572b0` (no load running, an idle fade, START
  pressed, no override from `FUN_1403ad540`, `DAT_142a0d628` clear) → `FUN_1402e4950(0)`. Nothing
  there tests the world map.
- The camp task `FUN_14031d3c0`: on the journal page (4) with the world map's heap loaded
  (`FUN_14038d050`), it loads the journal's files itself (`FUN_1402f5a40`: message slot 0x65)
  and sets the journal kind to 0 (`FUN_1402e4930(0)`).
- The landing grid's Triangle (logical bit 10, `FUN_1403928a0`) calls `FUN_14038e050(world)`:
  `FUN_1402e4bc0(0, FUN_1402f23d0(world))` stores the journal index in `DAT_1407434d8`
  (`FUN_1402f42d0`) and opens page 4 straight (`FUN_1402e4950(4, 0)`). The journal's open
  (`FUN_1402f3d70`) reads the kind (`FUN_1402e4780`): kind 0 starts on the world select page
  whatever `DAT_1407434d8` holds.
- Play time: `FUN_1401574e0` adds one tick a frame to the total (save +0x2444) and, while
  `DAT_140716884` is set, to the current world's (save +0x244c + 4 × world, world < 0x13), both
  capped at 0xcdfe5c4. The landing info panel shows the world's (`FUN_140157430`: hours,
  minutes, seconds).
- The Gummi route: deciding the route row closes the menu (`FUN_140398810`) and queues
  `FUN_140394690` through `FUN_1403946f0(route − 1)`: a frame capture (`FUN_140153430`,
  `FUN_140156780`), the route in `DAT_1409a8788` (`FUN_140151ba0`) and the Gummi game mode
  (the `GAME_MODE` object at 0x140716cb0, vtable 0x1405b14b8 → `FUN_1401505a0`, its task
  `FUN_140150310`).
- A refused deck command plays sound 5; a taken one plays none (`FUN_1403b21d0`).

## Steps and implementation

1. The field program's camp: the data the camp opens with and the journal's data are now
   functions (`camp_data` with a `CampSide`, `journal_data`), shared with the new
   `test_room::world_map_camp` host: it loads the camp, journal, item and ability sources from
   the field configuration the launcher passes the world map (`--field`), opens the camp on
   START, runs the camp → curtain → journal → camp chain (the journal as kind 0 on the world
   map), opens the journal straight for the landing grid and draws them. The settings pages
   save beside that configuration as in the field.
2. The world map: the camp's tick after the map's; while the camp or journal is up the map is
   not stepped (its pad still follows the buttons, so the closing press is not read again); the
   camp draws over the map's 2D, under the fade.
3. `landing::Change::Journal` from the grid's Triangle.
4. Per-world play time: the world map's own (15) counts with the total; the info panel shows
   the world's; `--world-time <world>:<h>:<m>` sets one as a save would hold it.
5. Unattended runs press Triangle and back in the grid and open and close the camp after the
   flight.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo build --workspace --locked` and the smoke run (see
  the PR for the counts).
- New unit test: the grid's Triangle asks for the journal.
- Installed unattended runs (`kh2fm wm … --touch 47,48,50,52 --world-time 2:3:21
  --play-time 12:34 --field <arena config> --autoplay …`): Triangle in the grid opens the
  journal on the world select page (sound 3), the back button closes it (sound 4) and the
  landing menu takes the next press; Twilight Town's panel shows PLAY TIME 3:21; START opens
  the camp over the map with TIME 12:34 (sound 3), the cursor moves (sound 1), START closes it
  (sound 4).
- NOT RUN: a comparison with the installed game (no save on this PC reaches the world map);
  the camp's pages one by one on the world map (they are the field's, T088–T164).

## Uncertainty

- The play time stops while the camp is up [I: `DAT_140716884`, which gates the per-world
  count, and the total's caller `FUN_140154700` were not traced further].
- The journal opened from the grid has no curtain [I: page 4's own open was not traced].
- The camp keeps the field's starting state (no save is decoded); Sora's model is not drawn
  on its party panel here.

## Not done

- The Gummi game mode: the route row ends the world map with the route logged; the shooter,
  its menus and garage are a separate mode of the executable.
- Carrying the camp's changes, the play times or the world map's state into the field the
  landing opens (each program starts from its configuration).

## Verdict

PROGRESS (no gate). Next: the Gummi mode; a comparison with the installed game once a save
reaches the world map.
