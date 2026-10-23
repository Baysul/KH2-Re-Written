# T212 — World map: lock-on, dash and warp, the cursor's deck, play time and fades

Owner Claude, 2026-10-23, requested by burba in the project chat ("let's work on the map system
please recreate that 1:1 using ghidra dont stop until its perfect"), handed over by the
project's coordinator session. Gate: development of the world map program only; no final
gameplay acceptance is inferred. Prerequisites: T171–T179 (the world map's state, screen,
landing, routes, events, scenes, effects, landing jump and gated landings), T170 (the field's
lock-on reticle), T202 (the world command layouts). Build identity: branch
`claude/project-thread-ofpsmp` from `main` at `8f7de1b`, merged with `main` at `b5f4230`,
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated each
run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/worldmap-1to1-20261023/`
(plain headless Ghidra 12.1.4, `analyzeHeadless -readOnly` on the analysed world map project of
T171; decompiles `out/d1`..`out/d18`, data dumps `out/dd1.txt`..`out/dd5.txt`, references
`out/refs1.txt`..`out/refs5.txt`, operand searches `out/fd*.txt`, the cursor's vtables
`out/vt-player`). The earlier world map decompiles under `research/worldmap-20261016/out` were
reused. Unattended runs and captures are in the private `kh2-worldmap-out/` (`base-01`,
`run-01`..`run-04`, `field-01`, `wall-*`).

## What the executable does

### The cursor's actions

- The action table (`FUN_140409d20`, records of 16 bytes at 0x140750950) gives ActionShip 0x2f
  (vtable 0x1405cd6c8), ActionWarp 0x3a (0x1405cd720) and ActionDash 0x3b (0x1405cd778).
- ActionShip: `FUN_140563880` picks motion 0x99 once the requested speed reaches the mean of
  the walk and run speeds, else 0, through `FUN_1403c6dc0` with an 8-tick blend
  (`DAT_140623bac`); `FUN_140563840` steers with the PRTY record's +0x28 / +0x2c.
- ActionDash: the enter (`FUN_140563a70`) sets the velocity to the move request (+0x10, the
  stick or the facing) times PRTY +0x24 (60 for the cursor's record 59), starts motion 0x9a at
  once and element 2 (`FUN_140395950`); each tick (`FUN_1405639e0`) asks for that speed and
  steers with 0.95 / 0.95 (`FUN_1405639b0`); when the motion has ended and the back button is
  up (mgr +0xc bit 0) it returns to the ship, whose exit (`FUN_140563a30`) blends to motion 0
  over 8 and releases the effect.
- ActionWarp: `FUN_1403978c0` builds a cubic Bézier from the cursor to the ground under the top
  of the target's sphere (`FUN_1403f3dd0`: a ray from 1000 above to 6000 below), the inner
  points at 0.1 and 0.9 of the way raised by 1000 and 500, over 30 ticks; `FUN_140397820` /
  `FUN_1403b6d70` run it; motion 0xa1 at once, no velocity (`FUN_140563950`).
- `FUN_140394900`: the floor's material (obj+0x780, via `FUN_1403b84e0`) starts element 5 on
  material 1 or 6 on material 2 and releases the other.

### The deck and the lock

- The cursor's command manager holds deck kind 10 (`FUN_1403f3ac0` → `FUN_1403b2800`).
  `FUN_1403947a0` rebuilds it every tick with one row: 0x246 (text 0x46a4, "Warp") while a
  target is held, else 0x247 (0x46a3, "Dash"), selector 1 (the back button). `FUN_14015df90`
  draws it with header group 8, row groups from 0x140718330 and icon 0xdb; the reaction slot
  (`FUN_14015d670`) shows the land or route command above it.
- `FUN_140394fc0`: 0x247 enters ActionDash; 0x246 kills the dash effect and warps.
- The lock manager (`FUN_1403dcd70`) skips the automatic target for the world map cursor (flag
  0x20000000 set by `FUN_140395420`). R1 searches with `FUN_1403bea30` (candidates' TARGET
  spheres within SSTM +0x4c = 1200 of the look point, the cursor 3500 along +Z per
  `FUN_140394d10`; the nearest to the cursor wins), plays sound 7 and makes
  worldmap::LockOnCamera (`FUN_1405635d0`); R1 again or a lost target plays sound 8 and
  releases (`FUN_1403dd510` → `FUN_140563430`). A right-stick flick moves the lock
  (`FUN_1403bd750`: cosine ≥ 0.5 from the current target, nearest) with a new camera.
- worldmap::LockOnCamera (`FUN_1405636c0`): it frames the point 15% of the way from the cursor
  to the target; the eye offset runs from (0, −1840, −2400) to (0, −3450, −3000) as the
  distance goes from 1000 to 6000; the target is 500 above; it blends in over 30 ticks.
- `FUN_140394810`: the info panel follows the object of the land or route reaction, else the
  locked target.

### Panels, fades and walls

- dk::WM_TOTAL_INFO (`FUN_14038e080`, tick `FUN_14038e8c0`, close `FUN_14038e7e0`): the total
  play time (save +0x2444 in ticks, `FUN_140157390`) as up to three hour digits, a colon and
  two minute digits, X origin −85; made at the area start and closed for good by the first
  landing or route menu.
- The info panels' sprites take X origin 0x55 (`FUN_140395970`, `FUN_140398ef0`).
- The map is reached with jump mode 1 (`FUN_140154e20` → `FUN_140152990(…, flags | 1)`) and
  the landing leaves with it (`FUN_1403946d0`): `FUN_140156520` fades to black over n = 15 and
  the jump waits for it (`FUN_1401563f0`), the input locked; the arrival (`FUN_1401561b0`)
  fades in over n = 15. Event jumps (ops 0x10e / 0x10f) pass 0x80000001: no fade out, the
  arrival still fades in.
- `FUN_1403a92c0`: a contact with an attribute-bit-11 wall (+0x732) stops the cursor and knocks
  it back along the wall's normal at strength 1000 (`FUN_140394cc0` → `FUN_1403d4960`) once per
  new polygon, latched 30 ticks; the map's category-1 effect plays there when the map has one
  (wm00 has no map PAX).
- Start opens the camp menu on the world map as in the field (`FUN_1401572b0` has no world map
  test); the landing grid's Triangle opens the journal's world page (`FUN_14038e050` →
  `FUN_1402e4bc0(0, journal index)`; the camp task loads the journal's files for it,
  `FUN_14031d3c0` → `FUN_1402f5a40`).

## Steps and implementation

1. `world_map::cursor`: the action constants, the ship's motion choice, the engine effect rule,
   the warp path, the lock search and switch, the look point and the lock camera's goal.
   `world_map::camera::LockOnCamera` adds the blend.
2. The world map program (`main/src/world_map.rs`): the lock manager, the deck's command, the
   three actions, the knock-slide off bit-11 walls, the floor's engine effects, the camera
   slots (event or close-up, lock, follow), the reticle on the `fiel` sheet of
   `field2d/us/wm0field.2dd`, the deck on `field2d/us/wm0command.2dd`, the play time and its
   panel, the 8-tick crossfade of the cursor's motions (`blend_poses`, as Sora's), the fade in
   at arrival and the fade out before a landing's jump. `--play-time <h>:<m>` sets the save's
   play time; `KH2RW_WM_FLY=<x>,<y>` steers an unattended run after its menu script.
3. `visual::world_map_menu::TotalInfo`; the info panels' X origin.
4. `visual::field_command_menu`: deck kind 10 (`WORLD_MAP_DECK`).
5. The field: an area config's `"arrival": 1` fades in from black over n = 15; the world map's
   landing config sets it.
6. `collision::World::polygon_normal`.

## Checks

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`
  and `cargo test --workspace --locked` passed (see the PR for the final counts).
- New unit tests on generated values: the deck row, the ship's motion, the engine effects, the
  warp path (lands on the ground, 30 ticks), the lock search and switch, the lock camera, the
  landing config's arrival.
- Installed unattended runs (`kh2fm wm … --autoplay`):
  - The play time panel shows 12:34 with `--play-time 12:34`, top left; the info panel sits at
    the right edge; the deck shows "Dash" with the "Land" reaction above; the landing menu
    hides the deck and closes the play time panel.
  - R1 near Twilight Town locks (sound 7), the deck turns to "Warp", the reticle and the lock
    camera show; the back button warps in 30 ticks to the ground under the target; the back
    button without a lock dashes at 60 for the motion's 120 ticks.
  - A decided landing fades out, then jumps; `kh2fm room` on the written config fades in from
    black over 30 ticks (mean brightness 4, 33, 70, 106 at ticks 2, 10, 20, 34).
  - The floor under the flights has material 0 (no engine effect); flights into the walls log
    the bit-11 contacts and their knock (see the PR).
- NOT RUN:
  - Any comparison with the installed game itself: no save that reaches the world map exists
    on this PC, so nothing here is checked against the original's screen.
  - A run with the hand-held controller (the flick switch and a held back button were not
    driven).

## Uncertainty

- The deck hides while the landing menu, an event or a jump holds the cursor [I]; the
  command draw's own hide rules (`FUN_14015e800`) were not traced to the end.
- The TARGET shape of Twilight Town's symbol is on bone 2; its centre is taken on the root
  [I], as the landing shapes are.
- The look point's +3500 is on Z as the decompile reads (`param_2 + 8`).
- The wall's normal is the polygon's plane normal (`FUN_140173500`); `FUN_1403ba190`, which
  gates the knock, was not traced.

## Not done

- The camp menu (Start) and the journal from the landing grid: the camp is hosted by the
  field program only.
- The per-world play time (save +0x244c) on the info panel: this program keeps none.
- Gummi missions after a route is chosen, the symbols' hit reaction, range triggers and the
  motion sound fade (from T174 / T177).

## Verdict

PROGRESS (no gate). Next: host the camp menu and the journal on the world map; a comparison
with the installed game once a save reaches the world map.
