# T217 — Limit Form follow-ups: shots and the map, Strike Raid's spin, the hop target, Zantetsu Counter

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep working on it until
there's nothing left to do"), with the coordinator session's list of open items from T210.
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T210 (Limit Form on the script engine), T216. Build identity: branch
`claude/project-thread-ww9y8z` from `main` at `d4d0c24`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`
(headless Ghidra 12.1.4 on a read-only copy of the analysed project: `out/h_dc8.txt`,
`out/h_11208.txt`, `out/h3`, `out/h5`); the `limitform` listing in `kh2-party-1to1/bdx`; runs
`kh2-party-1to1/runs/lfa1`..`lfa7`.

## What the executable does

- Infinity's shot thread (`limitform` L351..L791) homes each shot (`vector_homing` 0.087266 a
  tick, speed 16) on its target (`target_pos`), or without one on a point in the air around
  Sora's body bone (1:248 `bg_get_random_pos_air`, 100 above, which tests the map with
  `bghit_check_line`); it ends on a hit or when its time runs out (L1051). It has no ground test:
  a shot diving on a grounded target from Sora's height can pass below the floor.
- Strike Raid's `fly` thread turns the thrown Keyblade each tick with `obj_set_xyzrot` (8:2) from
  6:68's angles: a frame whose −Z is the travel and whose up the hint keeps turning, so the
  blade rolls about its travel.
- Sora's +0xdc8 is the target holder `DAT_142a11208` (FUN_1403a7a40 stores it; FUN_1403dcb30
  builds it; FUN_1403dcd70 / FUN_1403dd650 run its automatic and manual states); 6:67's target
  record is its +4.

## What the room does now

- The party-limit scene's map test `line_hit` is the room's segment cast (the map's own ray, or
  the arena's triangles), so 1:248's air points stay above the map.
- The thrown Keyblade takes the program's `obj_set_xyzrot` turn (the angles back to axes, the
  script frame to the room's) instead of a guess along its travel.
- The hop target stays the room's target holder; the code now cites the record.
- `--limit-form-tour` gains a counter window (ticks 360..680): the Shadow fights, Sora stands 60
  in front of it, guards when a claw starts and attacks out of the guard recoil; the limits
  follow from tick 700.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; the tour's counter schedule tested) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--limit-form-tour` (private `runs/lfa7`) | exit 0; the Shadow claws (457), Sora guards (458) and counters with PTYA 0x27 (motion 154, tick 531), hitting with ATKP 1877 (567); the four limits start and end with no faults (the form time topped up before each) |
| Thrown Keyblade (private `runs/lfa4`, trace) | the blade's long axis stays square to the travel (cosine 0.04) as the program rolls it |
| Unattended `--autoplay` (private `runs/lfa2`) | Infinity's shots that dive on the respawned Shadow pass below the floor, as the exe's thread allows (no ground test) |

## Uncertainty

- [I] The angles go back to axes as `euler_xyz` reads them (row-vector Rx·Ry·Rz); the
  Keyblade's pose is placed under them as under the hand.
- Not compared: the throw, the shots and the counter against the installed game's frames.

## Verdict

PARTIAL. The four items are settled from the executable; nothing is compared with the original
game's frames.

## Next task

Drive forms' particles and afterimage from each form's own effect data; the camp's Customize and
Abilities pages for Donald and Goofy.
