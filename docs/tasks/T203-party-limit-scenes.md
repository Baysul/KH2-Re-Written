# T203 — Party limits run from the second deck

Owner Claude, 2026-10-20, requested by burba in the project chat (follow-on to T202: "make
picking a summon, a Party entry and a party limit actually do their action after the menu
closes, recovered 1:1 from the exe"). Gate: development of the test room only; no final
gameplay acceptance is inferred. Prerequisites: T202 (the second deck), T186/T201 (the party
in the room), the `enemy` crate's object-script engine. Build identity: branch
`claude/project-thread-6zu7ov` from `main` at `3645961`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build,
validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/command-deck-20261020/`
(decompiles `out/d9`..`out/d37`, trap tables `out/t6`, `out/t7`, data dumps `out/dd5`..`dd7`);
script listings and lifts of the limit programs, probe output and the unattended runs in the
private `kh2-command-deck/` folder (`bdx/`, `runs/final4`).

## What the executable does

- Picking a Limit-list entry starts the limit (`FUN_1403d8b40`): all of Sora's MP goes
  (cost 0xff, MP Charge starts), the members not taking part fade out, and the limit object
  (`FUN_1403d7fa0`, 0x128 bytes, `DAT_142a24cc0`) loads `limit/<lang>/<file>.bar`
  (`FUN_1403d96d0`): the scene program (BDX type 3, trigger 22 = start, 23 = follow-up), the
  limit's own motion set (`LIMI`, keyed `"%03d0"` by objentry +7), its PAX, cameras and the
  gauge sheets. The time is the command's +0x20.
- The scene offers commands through `command_override` (`FUN_1403b2850`): record 0 takes
  the row of the Attack kind, record 1 is the reaction (Triangle) command, record 2 takes the
  Limit row (command 0x51), records 3–6 the whole deck (`FUN_1403b0f20`). The cursor keeps
  its row (`FUN_1403b11e0` follows the remembered command id).
- A command is taken only when `FUN_1403b6950` agrees: the command's +0x1f names whose
  motion windows decide (0 the user, 1 the override's target friend, 2 both), and
  `FUN_1403ba070` accepts inside a window the motion opens — range 0x14 (flag bit 17) for the
  command itself, range 0x1a (bit 21, value at +0x638) for the command whose +0x22 names it,
  or the free windows (ranges 0–2, flag bits 7/8/2 of +0xc) — and refuses under range 0x28.
- Each motion start (`FUN_1403c64a0`, also for a queued motion in `FUN_1403c6740`) gives the
  object a new token at +0x184; the scene waits on it to see a motion replaced.
- The gauge (`FUN_1403d9130`) runs down by 1.0 × the ability rate each frame while no
  participant's motion holds range 0x2d (+0x194 bit 0).
- Duck Flare keeps a flare pool (traps 7:3–7:11; `FUN_1405410e0`, 46 records of 0x50 bytes,
  free stack at +0xe60): every frame (`FUN_140541180`) overlapping flares part by half the
  push-out each (`FUN_1403c94c0` / `FUN_1403c90b0`, cylinder and sphere shapes, margin 5.0)
  and are pushed out of Sora's kind-4 shapes (his model's one sphere, radius 90, 115 up).
- Enemies are team 2 (`FUN_1403ab4a0`, `FUN_1403d0150` test +0x4dc).

## What the rewrite does

- `enemy::limit_scene::Scene` runs the program with Sora as the player stand-in and the
  friends and their weapons as engine objects; `enemy::traps::limit` holds the limit trap
  group (2:47–2:63, 2:89, 1:263), `enemy::traps::flare` the flare pool, `searcher` gains
  6:12 and 6:21, target records may live in script memory, `obj_can_decide_command` follows
  `FUN_1403b6950` (`traps::state::decides`).
- `test_room::party_limit` starts a picked limit, ticks the scene in place of Sora's own
  update, puts his and the friends' limit motions on screen, plays the file's effects, lands
  the scene's attack objects and direct strikes on the Shadow with the owner's stats, and
  logs `party_limit_start/offer/follow_up/tick/attacks/end` (`KH2_LIMIT_TRACE` adds every
  trap called).
- The command menu shows the offered commands on their rows with the override art; a taken
  row runs the limit's command and no longer opens the Limit list.
- `--party-limit-tour <dir>`: locks on, then for each Limit-list entry stands Sora before the
  Shadow with full MP (tour setup), opens the list, picks it, presses the offered command and
  the reaction button, and captures.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (round 2; the two `enemy::traps::weapon` tests now use the whole trap table and pass the object to 10:31) |
| `cargo build --workspace --locked`, release `--features skate3` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Unattended `--party-limit-tour` (private `runs/final4`) | see below |

Unattended tour, locked on to the Shadow (round 2, private `runs/final5`):

| Limit | Runs to its end | Commands taken | Hits on the Shadow |
| --- | --- | --- | --- |
| Whirli-Goof | yes | 283 ×2, 256 ×3, 257 | 23 |
| Knocksmash | yes | 339 ×4, 340 ×3, 341 | 6 |
| Comet | yes | 280 ×5, 281 | 23 (comets 664 ×18, finisher 665 ×5) |
| Duck Flare | yes | 347 ×4, 348 | 173 |
| Trinity | yes | 487, 488 | 2 (and one direct strike) |

No faults and no missing traps in any of the five.

Fixed in round 2 (all from the executable):

- `sysobj_motion_id` gives a limit motion's own id (+0x180, FUN_1403c63e0) and a limit
  motion sets +0x18c bit 14 (FUN_1403c65e0 → FUN_1403c8a40): Donald's Duck Flare stance
  loop (motion 252) now runs.
- A script may test `effect_is_active` in the tick it starts the effect: handles asked
  for during the tick count as running. Comet's impacts and Duck Flare's flares live.
- 6:23 `obj_near_parts`, `attack_strike` events carry the attack (scripts free it in the
  same tick), FUN_1403b6950's +0x1f decider and FUN_1403ba070's flag terms.
- Sora's side is not of the enemy class (+0x6c8 bit 4): `enemy_each` no longer walks the
  friends, weapons and the scene program (Knocksmash's faults).
- `obj_move_request` (1:89) is applied: FUN_1403b89a0 adds +0xa58 × +0xa64 to the frame's
  movement and clears it. Goofy's shield now flies to the target.
- Participants carry their objentry part (+0x4c = character, `obj_search_by_part`) and
  the friends the Shadow as their battle target.
- Trinity Limit: FUN_1403d96d0's special window (+0x54 bit 3, +0x50) with the mode-3 deck
  (FUN_1403b2a90 / FUN_1404065a0: rows 0x1e7, 0x1e6, 0x1e5), 6:43 (row → 0x1e8), 7:36
  (the combo counter's hits), 8:0 (TARGET radius), and the shot pool 7:28–7:31
  (FUN_140541a00, FUN_140541ba0, callbacks FUN_140541ac0 / FUN_140541a70 / FUN_140541d30,
  turn FUN_1403b7200) in script-writable memory.
- The tour presses Triangle while reaction commands come and a row's command once none
  has come for 240 ticks; the top deck's rows come back after a limit.

Round 3, the limit HUD: the gauge (YS::MISSION_GAUGE → dk::MISSION_GAUGE, FUN_14017b750 /
FUN_14018c990 / FUN_14018cf00 / FUN_14018d040: frame 3, fill 0xc scaled by time / start time,
rest 0xd, marker 0xe, cap 9; right side, slot 0xc) and the combo counter (YS::COMBOCOUNTER →
dk::COUNTER two-row mode, FUN_1403fa730 / FUN_14018be20 / FUN_14018c560: "%09d", digits 0xc..
and 0x18.., leading zeros 0x16 / 0x22; left side, slot 0xc, range 0–999) draw from the limit
file's own `gage` / `comb` sheets with their HD art (`visual::limit_hud`). Trinity Limit and
0x26c show no counter. Every hit on the Shadow during a limit counts. [I] A child sprite sits
at its parent's child context once (the executable also sets each child's offset to that
context; FUN_14016abf0's composition was not traced to the end).

Round 4, Trinity Limit's presentation (private `runs/r6e`):

- The shots draw. FUN_140541a00 builds the pool on object 0x5c3 (`PRIZE`); FUN_140411b90
  writes each record's 16-byte point (FUN_140412450, mode 0: position, +0x14 group, +0x15
  alpha, +0x16, +0x17 frame), and program 203 of `obj/PRIZE.mdlx` (FUN_1404d7ed0, read
  through FUN_1402bebf0 / FUN_1402c3300) draws group 7 per point, skipping alpha 0 and stepping
  the flip-book frame (0xff starts a looping shape at a random frame). FUN_1404118e0 and
  FUN_140541ba0 start a record at sprite 8, alpha 0x80, frame 0xff. 180–220 shots live
  during the combo.
- The command menu takes the limit file's `comd` sheet (FUN_1403d96d0 -> FUN_1403b2a90 ->
  FUN_1404065a0 -> FUN_14015de90: the IMGD becomes the menu's texture, the sequence +0x3a0)
  and, in deck mode 0xE (FUN_1403b1780), the list path FUN_14015df90: header group 8, rows
  by selector (0x140718330 + 0x14: 1 -> 0xa, 2 -> 0xc, 3 -> 0xb, 4 -> 9), text colours
  0x140717d50, no cursor. FUN_1403b1590 takes the row whose selector's face button is pressed
  (DAT_1405c4ed8: logical bits 24–27, Circle / Triangle / Square / Cross in MODE_FIELD):
  Begin Combo Triangle, Ultima Square, Break Cross.
- `visual::hud::load` takes the HD replacement whose original asset offset lies inside the
  IMGD (flag 0x2000_0000); trinity.bar keeps its PAX textures' replacements before its
  sheets' and a sound bank after them, so position alone named the wrong picture.

Still open: the state sprite each slot carries (FUN_14015d140 +0x220, 0x140717ea0), the
constants FUN_1404065a0 stores for mode 3 (DAT_142ae9ab0/ab8).

Round 5, the list rows' button glyphs (private evidence `command-deck-20261020/glyph/`):

- FUN_14015df90 in deck mode 0xE builds each row's label with FUN_14015aba0 when the row has a
  selector: selector 1 glyph 0xdb, 2 and 3 FUN_1402ebe70(0x1000) / (0x8000), 4 0xda.
  FUN_1402ebe70 looks the pad bit up in the table at 0x1405ba7e0 (0x1000 -> 0xdf, 0x8000 ->
  0xde, 0x2000 -> 0xdb, 0x4000 -> 0xda, ...): Begin Combo Triangle, Ultima Square, Break
  Cross.
- FUN_14015aba0 writes `11 X ff 1c 00`, `09 glyph`, `11 02 00 1c 00`, then the message
  (FUN_1403ae260). X follows the PLAYER_COMMAND's sheet mode (+0xab0, the first word
  FUN_1404065a0 stores; FUN_1403b17c0..FUN_1403b1820 test it): a limit's `comd` sheet is
  loaded with mode 3 (FUN_1403d96d0 -> FUN_1403b2a90), which gives 0xe2.
- In the text routine (FUN_1403b0280) 0x11 moves the pen to two signed 16-bit offsets from
  the text's origin (x = 0xffe2 = -30, then 2; y = 28) and 0x09 draws the pad icon
  (FUN_1403af9e0). The glyph sits 30 px left of the label and the label 2 px right of its
  usual place.
- The rewrite: `field_command_menu::glyph_label` and `label_quads` (control codes take their
  parameters, 0x11 sets the pen's x, 0x09 icons as `sprite_player::text_quads` draws them);
  the command menu loads the font's icon plane and draws the icons from it.
- Unattended `--party-limit-tour` (private `runs/g1`): exit 0; Trinity Limit's rows show
  their buttons inside the rows' circles (Xbox kind: Y Begin Combo, X Ultima, A Break) on the
  rows' line, the labels as before.

## Uncertainty

- Lift argument order: a routine's first popped slot is its last argument.
- Engine objects keep no command lockout (+0xb94); Sora's flag-only window terms are left to
  his kit.
- A move request is applied as a second collision step after the frame's movement.
- The special deck runs in deck mode 0xE: inferred from FUN_14015de90 being the only writer
  of the `comd` sequence (+0x3a0) that the mode-0xE draws read. Leaving the deck restores the
  menu's own sheet (the restoring routine was not traced).
- [I] 0x11's y (28, less the first line's height FUN_1403ae670 measures) is read as the
  line's own baseline: the icon and the label stay on the row's line. The deck struct's +8
  is read as the PLAYER_COMMAND whose +0xab0 the predicates test. The HUD's pad kind is the
  room's fixed one (1).

## Verdict

PARTIAL. All five party limits run start to finish in the test room with their commands and
hits, as far as checked against the executable; the presentation listed above is missing.

## Next task

The slots' state sprites (FUN_14015d140 +0x220) and the mode-3 constants.
