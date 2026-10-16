# T160 — Sora's level-up panels, level-up effect and bonus levels

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub issue
#18). Gate: development of the test room only; no final gameplay acceptance is
inferred. Prerequisites: T096 (EXP, prizes), T108/T113 (form level-up panels,
system effect 16). Build identity: `main` at `ac92a2d`, package 0.1.2, Rust 1.99.0
Windows MSVC. Source: the selected Steam KH2FM build, validated each run; the
install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/levelup-20261016/`
(Ghidra decompiles `out/d01`..`d08`, references `out/r01`, the raw `lvup`/`bons`
rows read by `probes/lv_probe.rs`, the gauge sequence groups printed by
`probes/gaug_probe.rs`, unattended tours `tour1`..`tour3`).

## What the executable does

- `FUN_1403ecf30` adds the kill's EXP (cap 9,999,999) and, for every party
  member, raises the level while the EXP reaches the next row. Per level it
  calls `FUN_1403ed7d0`, which lists "Strength / Magic / Defense Increased!"
  (sys 0xade..0xae0) and "Maximum AP Increased!" (0xae1) for every stat the new
  00battle `lvup` row raised over the previous one (Critical counts Sora's AP
  1.5x), then the growth ability of the character's chosen path (`+8 + path*2`,
  an item of type 0x13), which `FUN_1403c3f00` teaches. It queues a panel with
  the first two lines (`FUN_14017dd80`, queue A kind 0). The first level of a
  call also starts system effect 15 on the member (`FUN_1402c6b30`) and
  refreshes the stat object (`FUN_1403c2100`). Level-ups do not heal.
- A level row has no HP, MP or Drive. Those grow through **bonus levels**:
  `FUN_1403edca0(id)` runs once per id (flags `0x1409acfb4`, count
  `0x1409acfb0`) over the 00battle `bons` rows of that id and for each row
  `FUN_1403edf90` adds Maximum HP and MP (halved for the party on Critical)
  and heals the same amount, adds Drive bars (to nine), Item / Accessory /
  Armor slots and up to two abilities, then queues a panel (queue A kind 1)
  from the first two of "Maximum HP / MP Increased!", "Drive Gauge Powered
  Up!", "Gained Item / Accessory / Armor Slot!" (0xae2..0xae7) and ability
  lines. The id comes from the battle record at battle end (`FUN_1403a41e0`),
  an enemy record (`FUN_140410af0`, +0x5a) or a script.
- Queue A's panels are the gauge SEQD groups 0x68/0x69 (Sora), 0x64/0x65,
  0x66/0x67, 0x6a/0x6b (level-ups) and 0x54/0x55, 0x50/0x51, 0x52/0x53,
  0x56/0x57 (bonus levels) at (485, 20), anchored to the right edge, stacked
  down by +44 / +64, a batch of up to four at a time. Text: title right-aligned,
  level at the group's +0x20, line 1 at (−200 / −175, +19), line 2 17 below.

## Rewrite

- `data::battle_tables::bonus_rows` reads `bons`.
- `visual::level_up` now holds both queues (`Panels`, `Kind::{Form, Level,
  Bonus}`) with the groups, anchors and limits above; panel text uses the same
  top-of-line layout as the menus (the earlier lift of one cell was removed).
- `test_room/level_growth.rs`: the level lines, the panel texts, bonus levels
  (Sora's max HP / MP / Drive / abilities change; Donald's and Goofy's rows
  only show panels), the `--growth-tour` capture run and F3 "Bonus level".
  `prize_room::grant_exp` queues one panel per level and the level-up effect
  (system element 15, with its own sound request) is requested from `hud_tick`.

## Checks

- Unit tests: `data::battle_tables` (bonus rows), `visual::level_up` (groups,
  anchors, limits), `test_room::level_growth` (lines, Critical AP, groups).
- `--growth-tour` against the installed build: EXP to level 8 queues six
  panels (LV 2–5, then 6–8 after the first batch), element 15 starts with its
  sound; bonus 3 raised max HP 20 → 25 and learned ability 273, bonus 51
  raised max HP to 85 and max MP to 140, and the bonus panels (Sora, Donald,
  Goofy and a fourth character) showed. Not compared with installed-game
  captures.

## Not reproduced / uncertain

- The `dk::Exp` popup the kill also queues (`FUN_14017ca70`) is not drawn.
- The level-up effect (element 15) reports an unsupported program in the
  effect runtime, so part of its drawing is missing.
- Panel ability lines have no icon glyph; panel text size and positions follow
  the exe's formulas but were not checked against installed-game captures.
- Growth path 0 is always used (no save picks one). Drive is already at its
  nine-bar limit in the room, so the Drive gain shows no change.
- Bonus ids come from the F3 row: the test room has no encounter record or
  boss enemy carrying one (no ENMP record in this build names a bonus).
  Item, Accessory and Armor slots are shown but the room has no slot counts.
- Donald's and Goofy's levels and stats are not modelled (issues #8–#12).

## Verdict

PARTIAL. Next task: the EXP popup, then encounter bonus ids once battles exist.
