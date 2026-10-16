# T161 — Prize sprites, the heal effect and item treasure boxes

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub issue
#17). Gate: development of the test room only; no final gameplay acceptance is
inferred. Prerequisites: T096 (prizes), T150 (effect runtime, prize sounds).
Build identity: `main` at `1303a2b`, package 0.1.2, Rust 1.99.0 Windows MSVC.
Source: the selected Steam KH2FM build, validated each run; the install was
only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/levelup-20261016/`
(Ghidra decompiles `out/prize/`, the PRIZE DPD / shape / texture dumps by
`probes/prize_probe*.rs`, tours `prize1`..`prize3`).

## What the executable does

- A prize is a pool actor (`FUN_140412d40`) whose draw is **program 203** of
  `obj/PRIZE.mdlx`, a one-element PAX (program record `0x14077d970`, draw
  `FUN_1404d7ed0`): sprites from the DPD's four shapes. The program's
  parameter block is a four-byte header and ten twelve-byte groups (shape id,
  colour bytes, float scale); a prize of kind k uses group k: kinds 0/1 (HP)
  shape 1 scale 0.7/1.5, 2-4 (munny) shape 0 scale 1.2/0.8/0.4, 5/6 (MP)
  shape 2 0.5/0.9, 7/8 (Drive) shape 3 0.9/1.7. Each prize steps one
  flip-book frame per tick; the shape's last sequence entry says whether it
  loops (random start) or holds its last frame.
- Collecting an HP, MP or Drive prize runs `FUN_1403dca50`, whose first step
  `FUN_1403d7bd0` starts **system element 6** on Sora (bound to his object)
  unless it still plays, and stores the table's duration (30 ticks small, 60
  big, table `0x1405c7420` float +0); a collection while it plays only extends
  the time. The per-tick handlers count `+0xd40` down by one and clear the
  effect at zero.
- An item drop (`FUN_1403f8170`) creates a treasure-box actor of object
  0x3b8..0x3bb (`PRIZE_BOX_YELLOW/RED/BLUE/GREEN`) with a random heading; the
  item record's byte +0x16 indexes the table at `0x1405c96c0` (twelve
  {object, scale} rows: red, green, blue, yellow, three sizes each). Within the
  default pickup range (120 horizontal, 30 vertical, `FUN_1403dc500`)
  `FUN_1403f7df0` calls `FUN_1403f8770`, which adds the item
  (`FUN_1403c4d50` / `FUN_1403c3f00`) and sends the box to Sora.

## Rewrite

- `effect::prize` reads program 203's groups and builds the sprite quads;
  `drive_fx` draws them as billboards at each prize's centre. The placeholder
  coloured spheres were removed.
- The heal effect starts from `prize_room::tick` (system element 6, bound to
  Sora) with the 30 / 60 tick rule.
- `enemy_prizes::ItemDrop`, `box_style` and `data::item_table::Item::box_style`;
  `prize_room` spawns a box per successful item roll (when the item has a
  style), it falls and rests, and picking it up adds the item to the stock.
  `item_drop_view` shows the four box models.
- `--prize-tour <dir>` (unattended): a prize of every kind lands round Sora,
  four boxes land ahead, the HP / MP / Drive prizes are then moved onto Sora.

## Checks

- Unit tests: prize flip-book frames, heal durations, box styles and the
  drop pickup range. fmt, clippy (warnings denied), `cargo test --workspace
  --locked` PASS.
- `--prize-tour` against the installed build: prizes drew with the right art
  (green HP orbs, gems, blue bubbles, orange swirls); collecting started
  element 6 once per extension; item boxes of both shapes appeared and
  `item_pickup` was logged. Not compared with installed-game captures.

## Not reproduced / uncertain

- The prize sprites' texture coordinates are 12-bit fractions of the whole
  texture (4096 = 1.0): read off the installed atlases, not from the draw
  routine's arithmetic [I]. The group scale is applied to the sprite size [I].
- The prize and box physics (`FUN_1404132d0`, the box actor's state 0xc) are
  not traced; both use the actors' gravity. The box's fly-to-Sora pickup and
  its pickup popup are not drawn; the box model's local axes and size look
  tilted/small against the room and were not checked.
- Only the first item-table style 0..11 is mapped; the item add does not show
  the pickup message.
- Element 6's own drawing depends on the effect runtime's coverage; the tour
  logged no unsupported program for it.
- The heal effect on Donald and Goofy (their own prize pickups) is not built.

## Verdict

PARTIAL. Next task: trace the prize and box physics, the box pickup flight
and message.
