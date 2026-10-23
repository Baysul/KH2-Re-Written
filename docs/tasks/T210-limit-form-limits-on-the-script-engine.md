# T210 — Limit Form's limits on the script engine

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep working on it until
there's nothing left to do", following "finish all summons, limits, and party stuff until its 1:1
using ghidra"), handed over by the project's coordinator session.
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T207 (summon limits, party follow-ups), T203 (party limit scenes), T113 (Limit
Form), the `enemy` crate's object-script engine. Build identity: branch
`claude/project-thread-ww9y8z` from `main` at `9df64ae`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`
(headless Ghidra 12.1.4 on a read-only copy of the analysed project: decompiles `out/d16`..`out/d22`,
cross references `out/x3`, `out/x4`); the `limitform` listing, the Limit Form PTYA dump and
unattended runs in the private `kh2-party-1to1/` (`bdx/`, `runs/lf1`..`runs/lf12`,
`runs/lft1`..`runs/lft4`).

## What the executable does

### The program

- Limit Form's four limits have no `limt` row or file of their own. Their program is the BAR
  entry `limi` of `obj/P_EX100_KH1F.mdlx` (`limitform`: triggers 17, 22 and 23). The limit
  start (`FUN_1403d8b40`) runs trigger 22, which dispatches on `limit_start_command` (7:27).
- The program acts only for the Limit Form Sora: its check (L1604) needs `obj_objentry_id` in
  0x95d..0x961 (P_EX100_KH1F and its world variants).
- Sonic Rave and Ars Arcanum offer their follow-ups from the program (L5644: `command_override`
  of W28[0] while W36 > 0 and range 16 is open, W28[1] at W36 = 0 and range 17). Infinity's
  offers are its motion's range 20 records (246: 684, 685, 686, 684 over frames 42..100;
  `FUN_14040d810` case 0x14). The press reaches the program's trigger 23.
- Infinity's swings are `obj_party_attack` (6:67) on records 0x34..0x38 of the form's PTYA set
  (66). Shoot's shots come from trigger 17 (frame event 7 of motion 247, p0 = 100), the
  program's handler for Sora's frame events.

### `obj_party_attack` (6:67)

- `FUN_14053ef20`: for a player-class object (+0x6c8 bit 7) the record is found by id
  (`FUN_1403d74e0`) and run (`FUN_1403d78d0`) against the target record at +0xdc8 + 4
  (`FUN_1403bde30` checks it).
- `FUN_1403d78d0`: the target record is copied to +0xbf8; the record's motion starts with
  `FUN_1403c86a0`. A record with a hop (+0xc > 0) takes the height to the target clamped to
  [+0x14, +0x10] (+0x10 raised by `FUN_1403b5a40` when flag 2 is set) and the speed
  `(distance − both TARGET radii + +0x3c) / FUN_1403b73f0(height)` clamped to [+0x18, +0x1c],
  then jumps through physics vt+0x18 (+0x11c = 5). Without a target: the record's height, the
  facing and the top speed. Last, the record's next motion (+0xa) is queued with blend +0x38
  (`FUN_1403c8310`).
- Infinity's records: 0x34 motion 242 (hop 60, 30..200, speed −8..8, reach 85), 0x35..0x38
  motions 243..246 (hop 40, 20..80, the same speeds; 0x38 speed 0..0).

### The motion queue

- `FUN_1403c86a0` (1:11 and every motion start of 6:67) zeroes the queue's read and write
  indices (+0x50) before starting: what was queued behind the old motion is dropped.
  `FUN_1403c8310` (1:13) appends to a four-entry ring.

### Turning

- `obj_wish_dir` (1:4, `FUN_1403b65c0`) stores `atan2(x, z)` at +0x7a8; the object turns toward
  it at its turn rate. Sonic Rave turns Sora round before each dash this way.
- Range 28 (`FUN_14040d810` -> `FUN_1403d4b10` -> `FUN_1403d3a90`) and frame event 21 turn
  toward the target by up to the parameter in degrees, only beyond 50 units horizontally.

### Zantetsuken and Zantetsu Counter

- The form's PTYA set 66 holds the finisher record 3 (kind 3, flag 4 finisher, motion 154) and
  the guard-recoil record 0x27 (motion 154). Neither carries an ability bit (+0x40 = 0), so no
  code gate applies: the finisher ends Limit Form's ground combos and 0x27 is the attack out of
  the guard recoil (motion 42), the room's existing Counterguard path.

## What the room does now

- `party_limit` loads Limit Form's four limits from the form's `limi` with no `limt` row; Sora's
  object is P_EX100_KH1F's (0x95d) in the scene, plays his own motions by id (the form's kit
  keeps each scripted motion's trigger tables) and his locomotion 0..5 (the fall the program
  queues after an air finisher), turns at his PRTY turn speed, and the effects come from the
  form's container. The tint follows the limit's filter light and ramps out over 30 ticks after.
- Range 20 of Sora's motion offers its command as the reaction command; Sora's frame events 7
  reach the program's trigger 17 (`Scene::player_events`).
- 6:67 runs the record with its hop toward the room's selected target; 1:11 empties the queue;
  the scene's jump holds its height under a gravity range and falls from there when it ends
  (`FUN_1403cb500` / `FUN_1403b4780`, as the player's); airborne scene movement leaves the floor.
- Range 28 and frame event 21 turn Sora's scene object toward his target.
- Each motion the scene starts is a new attack for Sora's swing volumes.
- The T113 hand translation (`player::limit_form::Limit`) is removed; the commands, the row
  colour and the tint ramp-out stay.
- `--limit-form-tour <dir>`: drive into Limit Form, a ground combo on the Shadow, then Sonic
  Rave, Ars Arcanum, Strike Raid and Infinity, each follow-up taken as soon as it is offered.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new: 1:11 drops the queue, a gravity range holds a jump, range 28 turns Sora, the tour's schedule) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--limit-form-tour` on `KH2RW Launcher.exe` (private `runs/lft5`) | exit 0; the four limits end with no faults or missing traps. The combo ends on record 3 (motion 154, ATKP 1877 hit). Sonic Rave 698 → 699 ×5 → 700, 7 hits (1878 ×6, 1879). Ars Arcanum 701 → 702 ×5 → 703, 13 hits (1883..1895). Strike Raid 704 → 705 ×3 → 706, the Keyblade thrown along Sora's facing, 5 hits (1866 ×4, 1867). Infinity 683 → 684 (Shoot), swings 1857..1859 and 10 shots (1862). The finishers' EXP levels the form up (`form_level` at 1660) |
| Unattended `--autoplay` (private `runs/lf12`) | exit 0; Infinity and Strike Raid in the general run start and end; Shoot's shots strike the Shadow |

## Uncertainty

- [I] The hop target is the target the room selects (locked or not); the executable reads
  +0xdc8 + 4. The target's point is its feet, not its TARGET shape's centre. A record without a
  hop sets no ground velocity (Limit Form's have none), the jump-height raise of flag 2 is not
  added, +0xd74 is not set.
- [I] Range 28 / frame 21 turn only Sora's scene object; other objects still report them.
- [I] Infinity's shots fly through the floor when they home; their floor contact is not traced.
- [I] The program's `obj_set_xyzrot` angles of the thrown Keyblade are not applied (it is drawn
  along its travel); hit callbacks and hit sparks of the scene's attacks are not run (as T203).
- Zantetsu Counter (record 0x27) is not exercised by a run: the tour's Shadow does not attack.
- Not done here: equipment and Customize gating of Donald's and Goofy's abilities (#77); a tour
  of Sora's party items.
- No comparison against the installed game's frames or timings; no owner playtest.

## Verdict

PARTIAL. Limit Form's four limits run on the object-script engine from the form's own program
and PTYA set, with their offers, follow-ups, swings, hops, shots, thrown Keyblade, tint and EXP;
the hand translation is gone. Zantetsuken is the form's PTYA finisher and plays in the room.

## Next task

Equipment and Customize gating of Donald's and Goofy's abilities (#77); a tour of Sora's party
items; a controlled comparison of the four limits against the installed game.
