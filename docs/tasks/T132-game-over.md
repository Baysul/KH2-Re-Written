# T132 — Sora's game over

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub
issue #19, game-over flow). Gate: development of the test room only; no final
gameplay acceptance is inferred. Prerequisites: T094 (Sora's hit reactions and
DEAD), T099 (event camera/fade research), T109 (the 2LD menu renderers).
Build identity: `main` at `1e60690`, package 0.1.2, Rust 1.99.0 Windows MSVC.
Source: the selected Steam KH2FM build (manifest in the private test-room
folder), validated each run; the install and its saves were only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/game-over-20261016/`
(Ghidra decompiles `out/d01`..`d29`, reference lists `out/r01`..`r10`,
disassembly `out/a01`..`a04`, the extracted `pause.2ld` renders `render1`,
`render2`, and the unattended tours `tour1`..`tour4`).

## What the executable does (Ghidra, read-only project copy)

- Sora's death handler `FUN_140404d60` plays the DEAD motion (`FUN_140405a00`)
  and, outside a Drive form and special battles, creates the game-over task
  `FUN_1403fc7a0` mode 0 (`FUN_1403fcd20`). A KO in a Drive form or Antiform
  takes `FUN_14040cd80` instead (a different task, not built here).
- The constructor plays se000 sound 0x13, fades the sound masters and the field
  BGM, and starts the **death event** `FUN_1403f4b10(0)`: the first type-22
  entry of 00battle.bin (project name `re_gameover-0a`).
- That event holds: `EndFrame` 21, a camera cut with eight channels over frames
  0–30 whose target stays at (2.9, 123.2, 3.6) in Sora's local space
  (`FUN_140406a90` copies his matrix into the event), `SeqBlur` alpha 56 from
  frame 0 to 21, `SeqGameSpeed` 0.1 at frame 4 and 1.0 at frame 20, and a
  `SeqFade` at frame 11, duration 50, type 2 (white, layer A of
  `FUN_140156dd0`).
- State 2 of the task (`FUN_1403fcec0`) waits for the event to end and for
  layer A to hold opaque (state 4), then cleans the battle (`FUN_1403ac2f0`,
  `FUN_1403de010`), loads `obj/GAMEOVER.mset` (`FUN_1403fd2b0`) and opens the
  mode-8 list `FUN_1402e4c20` with pause definition 1 (14mission.bar `slct`
  row 1: Continue = result 2 / message 0x8ab0, Load Game = result 1 / 0x8aaf;
  backdrop group 3, no panel, flag bit 0 set so Cancel and Start do nothing).
- The list is `pause.2ld` SEQD 0 on image 0 at priorities 0x7531 (backdrop
  group 3: white to black over 16 frames, then black), 0x753b (model
  placeholder, group 16), 0x753c (rows: group 12, the selected row group 11,
  pitch 32 about the anchor (256, 216)) and 0x754a (glove group 6, light
  groups 7/8). Sora's model plays the set's motion `%03d0` of his weapon joint
  (joint 1 gives `0010`, the third entry of that name in entry 0xfc `AK10`,
  228 bones, 150 frames at 30 fps) at the menu model slot (translation z
  −100, the camp's model paste path).
- Continue (`FUN_1403fcbe0` with result 2) fades to black (`FUN_140152990`
  type 1, 30 ticks), restores every member's HP (`FUN_1403a5650`) and reloads
  the room at the last entrance (`FUN_1403a0660`'s record).

## Rewrite

- `data::pause_definition` reads the `slct` rows; `visual::pause_definition`
  is the list (backdrop, rows, glove, light, model slot, sounds 3/1/2, last
  row kept) with unit tests.
- `test_room/game_over.rs` runs the sequence: death event timeline (speed
  accumulator, white fade, blur weight, local camera), the list over
  Sora's lying pose, Continue and the reload; `--game-over-tour` and F3
  "Knock Sora out" drive it. The room's own KO refill is suppressed meanwhile.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` and `python -m unittest discover -s tools -p test_progress.py`: PASS.
- Unit tests: `data::pause_definition` (2), `visual::pause_definition` (4),
  `test_room::game_over` (4): PASS.
- Unattended tour `tour4` against the installed build: death at tick 90,
  event frames, white-out holding at real tick 331, list at 334 (backdrop
  flash, Sora lying, glove, light), Continue at 485, fade, reload: PASS for
  the presentation chain; captures were compared against the 2LD render only,
  not against installed-game captures.

## Not reproduced / uncertain

- The camera's local-to-world mapping (x mirrored, +z forward) is inferred;
  the game's own capture was not compared.
- The slow motion steps the field on an accumulator (one tick in ten); the
  original runs sub-tick time, so the fall stutters.
- The sound masters' and field BGM's fade-out, any game-over music and the
  `FUN_1401dd4e0` SE lock are not modelled.
- Load Game does nothing (needs the save screens, issue #32).
- The model's anchor and paste tint are the camp's; the list's exact text
  offsets were checked only against the art.
- Continue restores Sora's HP and puts him at the area spawn; party members
  have no HP yet (issue #11), and the room is not re-decoded.
- The Hades/retry (mode 2), mission-fail (mode 3) and Once More / Second
  Chance (`FUN_1403dc6d0`) branches, and the Drive-form KO task, are not built.

## Verdict

PARTIAL. Next task: save/load screens (#32), then party KO and revive (#11),
then the remaining game-over modes.
