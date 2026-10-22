# T204 — Summons from the second deck

Owner Claude, 2026-10-20, requested by burba in the project chat (follow-on to T202/T203:
"then look at summons"; routines recovered with Ghidra MCP on the installed executable).
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T202 (the Summon list), T203 (limit engine), the `enemy` crate's
object-script engine, the enemy gallery's model loading. Build identity: branch
`claude/project-thread-6zu7ov` (PR #117), package 0.1.2, Rust 1.99.0 Windows MSVC, release
built with `--features skate3`. Source: the selected Steam KH2FM build, validated each run;
the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/command-deck-20261020/summon/`
(decompiles `s1`..`s4`, the kn::Friend spec and decompiles in `friend/`); Ghidra MCP session on
a copy of the analysed project (private `kh2-command-deck/ghidra-mcp1`), then (from round (b)'s
`s4`, at burba's request) the headless runner `gh.cmd` there; summon script listings in the
private `kh2-command-deck/bdx`; unattended runs in the private `kh2-command-deck/runs/s1`..`s23`.

## What the executable does

- Execute 72 (`FUN_140405440`) for a Summon-list entry: Sora plays motion 0x8a (blend 8.0,
  0x140623bac), the command's cost (+0x1c, 3) is paid in Drive bars (`FUN_1403c2780`), and
  `FUN_1403de460` starts the summon: summon state `DAT_142a24edc` = 2, Sora held (player +0x120
  |= 0x104), Genie's form picked at random from the owned ones, and the task `FUN_1403de610`.
- `FUN_1403de610`: the party leaves (`FUN_1403e6fa0` → `FUN_1403e0460`), the task waits SSTM
  +0x88 frames (+0x84 for the first-time `appear` event, chosen by the summon effect setting
  `FUN_1402e4fa0`), the Drive gauge enters summon mode (`FUN_1403c2740`: mode 3, level + 3
  bars of SSTM +0x68 time; the level is `DAT_1409acdd6`, raised by `FUN_14040d530`), the
  `sumn` row's object (+4) is built as a kn::Friend (`FUN_1401b06e0`) of the summon class
  (+0x6c8 |= 0x400000) at a point 200–300 units from Sora (`FUN_1403f3fa0` /
  `FUN_1403f4180`: eight directions π/4 apart from a random base whose octant steps each call,
  a floor ray, a wall line, 100 nearer per round), Sora plays 0x82 and the object starts its
  act `appear_skip` (`FUN_1403cac50`). Sora is released and the state stays 2.
- Dismiss (execute 316, `FUN_1403de010`; also `FUN_140404a40` when the summon time reaches 0
  and Dismiss could run): state 3, gauge mode 1, task `FUN_1403de080` starts the act `leave`
  and waits for the object to go; the summon class's destructor (`FUN_1403dd850` /
  `FUN_1403dde30`) clears `DAT_142a24ee8` and the state; the party returns
  (`FUN_140400440`).
- The second deck's row 1 is Dismiss while the state is not 0 (`FUN_1403f3860`).
- The summon time drains in `FUN_1403a92c0` (× Summon Boost) and refills from hits ×12.

### Round (b): kn::Friend

- The friend object's update (`FUN_1401b15f0`) runs the personality's think (+0x380; default
  `FUN_1401b53b0`, Chicken Little `FUN_1401cb050`, Stitch `FUN_1401ca240`) and then the act
  tick (`FUN_1403b4f80`), and only while motion flag word +0x18c has 0x1c or bit 30 (the AI's
  free window). The think senses (`FUN_1401b8910`: Sora's block, the leash, a target by the
  personality's selector `FUN_1401b6b80` / `FUN_1401b3520` / `FUN_1401cbe50`), follows Sora or
  the target (`FUN_1401b41a0`, flight `FUN_1401b6360`, smoothing `FUN_1401b1b20`), and picks
  an action (`FUN_1401b1d40`; Chicken Little and Peter Pan "battle", Genie Follow and
  HealPlayer below a quarter of Sora's HP, `FUN_1401b3a30`), whose label starts an act
  unless a higher one runs (`FUN_1403caa90`). The friend keeps its choice in the object's
  target record (+0xbf8).
- Sora's attacks notify the friends (`FUN_1401b56e0` mode 1: the attack, its clock and its
  target; bit 9 of both friends cleared); `FUN_1401b5d60` ages the notice each frame and drops
  it when Sora's attack changes. `peterpan_receive_notify` (10:25, `FUN_1401cbdf0`) holds for
  8 frames until the friend takes it (10:26); Peter Pan's script then starts `attack_small`
  (within 300) or `attack_empty`. Hits on Sora go to an attacker list (mode 0).
- Chicken Little's fireworks (`P_EX360`, made by `sysobj_appear` inside his script) find him by
  entry (0x6c8), keep their effects on his watch list (10:53 `FUN_1401ce1e0`, 10:54
  `FUN_1401ce410`, 32 slots, ended effects swept by `FUN_1401ce050` each frame) and strike
  enemies within 240 of the burst (`attack_strike`, 2:17).
- An object in the ground mode that is off the floor and not rising while a gravity range
  holds (`FUN_1403b9090`'s leave-ground branch, `FUN_1403ba1e0`) falls: the class's handler
  ends in `FUN_1403b7850` (mode slot 4 with no height, `FUN_1403cbc30`, mode 1); the summon
  class first changes to motion 4 (`FUN_1403d6430`). Chicken Little's `surprise` lifts him on
  root motion and this brings him down.
- `btlobj_target` (2:23, `FUN_1404319d0`) searches again only for an enemy-class object
  (+0x6c8 bit 4) whose record kind is not 3 (`FUN_1403be2b0` kind 2 is Sora).

### Round (e): the summon limits

- A summon's script offers its limit on the second deck's Limit row (`command_override`
  record 2, `FUN_1403b2850` from 1:165 with priority 100); execute 81 starts it
  (`FUN_1403d8b40`). Its `limt` row has the summon as second participant (200–204,
  `FUN_1403e8300`): `FUN_1403d7fa0` marks Sora and the summon (+0x124 bit 16, physics mode 0 as
  `FUN_1403e8180` asks), and the program is the summon's own script: trigger 22 with the limit
  object (`peterpan.bar`, `stitch.bar`, `chickenlittle.bar`, `genie_*.bar` hold only the gauge
  and combo sheets). The trigger-22 code clears the act, calls `friend_start_limit` and runs
  the start command (`limit_start_command`, 7:27: the limit object's own command) through the
  same handler as its follow-ups (trigger 23); a limit thread (`limit_start_thread`) offers
  the follow-ups each frame while the gauge lasts (Genie: 543 on record 1 when Sora's windows
  take it, record 0 always).
- `player_set_exec_rc` (1:263, `FUN_14042fee0` -> `FUN_1403aaa40`): the command Sora runs.
- 10:29 `friend_use_item` (`FUN_1401b7d90`: a command on a target), 6:36 `obj_detach_camera`
  (`FUN_1403cc3b0`).
- The summon limit file's `gage` and `comb` sheets draw the LIMIT GAUGE and HIT COMBO counter as
  a party limit's do (`FUN_1403d96d0`); the summon's landed hits count toward the combo.

### Round (f): the limits' movement modes

- 6:40 `obj_limit_peterpan_idle` (`FUN_14053e900`, in Peter Pan's script) puts the AI's
  object (Sora in his limit) in physics mode 0x34 (mode object `0x140750c58`, flags 0x10a).
  Entry `FUN_14053da60`: the movement block from PRTY record 0x3e (`FUN_14040a480`), no
  velocity, +0x108 = +0x140. Velocity `FUN_14053d850`: with a request whose x and z are both
  nonzero the wished heading (+0x7a8) is its atan2 (`FUN_1403b65c0`); horizontally the forward
  vector (`FUN_1403b56e0`) times the run speed (+0x12c); vertically the request's y × speed ×
  (1 − inertia) + inertia × the last vertical speed; times the step. Exit `FUN_14053d9b0` loads
  PRTY record 1.
- 6:26 `obj_limit_hover` (`FUN_14053e1a0` -> `FUN_14053d680`) and 7:21
  `obj_limit_hover_set_spec` (`FUN_14053f880` -> `FUN_14053d660`) are called by Aladdin's
  script (`P_AL010`), not by a summon's. 7:21 copies nine words to `DAT_142b59470` (walk, run,
  turn, steering inertia, fast and slow inertia, the speed between them, hover height, climb
  speed); 6:26 writes the call's two inertias into it, sets the velocity to the vector's
  direction × w (NaN lanes 0) and enters mode 0x2a (flags 0x100). Its entry
  (`FUN_14053d9e0`) takes walk, run and turn from the spec, its exit (`FUN_14053d980`) the
  object's own record (+0x148). Velocity `FUN_14053d720`: a slower request is raised to the
  current speed (`FUN_1403b9da0`); `FUN_1403b7ad0` steers toward the request (blend
  `FUN_140141ae0`, the steering inertia kept) or keeps 1 − fast/slow of the velocity without
  one; the heading turns toward it (`FUN_1403b9f70`); `FUN_140540470` casts from 200 above to
  10200 below (10000 without a floor) and moves the height `climb` × step toward the spec's
  when it is more than that plus 10 off.
- 10:51 `chickenlittle_get_nearest_target` (`FUN_140537060` -> `FUN_1401cb360`, called by
  Chicken Little's script with a 120 × 60 box): the friend's aim point (+0x40, the one 10:5 /
  10:59 read and write) and each enemy TARGET go through the view matrix (`FUN_14019ce40`)
  and are scaled by the projection's +0x40 (`FUN_14019d010`) over their depth; the record
  names the nearest one inside the box, or is cleared.

### Round (g): Sora's motions in a summon limit

- No summon script starts a motion on Sora's stand-in; they capture one for him:
  `obj_motion_capture` (1:29, `FUN_14042d160` -> `FUN_1403c6cc0`) with the summon as captor
  and Sora as victim, slots 252..256 of the summon's own set (Peter Pan 252, 253, 255, 256;
  Chicken Little 252, 253; Genie's object 252; Stitch's script none).
- `FUN_1403c6cc0` resolves the slot through the captor's set (`FUN_1403c7c50`, direct for these
  sets) and hands the slot's archive to `FUN_1403c64a0`, which starts it on the victim's body
  (+0x158) and, for an object with +0x6c8 bit 2, on its two weapon parts (+0xd60). The slot's
  archive is a set of its own: entries "0010" (body) and "0011" / "0012" (the weapons), four
  variants each.
- The rewrite: `summon::capture_motions` loads Sora's body and Keyblade halves of those slots
  into his kit (ids 0x7800 | row << 8 | slot − 240); the summon brain answers a captured
  slot's length from them (`ClipView`), so the capture lasts its motion; `sora_in_limit` plays
  the stand-in's capture.

### Round (d): Genie's forms

- `FUN_1403de460` gives Genie one of Sora's owned forms (Valor 1, Wisdom 2, Master 4, Final 5;
  Anti never) by the 69069 generator (DAT_140783ca0); Peter Pan's form is 1 (DAT_1409acdd7).
  Genie's first update (`FUN_1401cf370`) takes the form's command (`FUN_1403de220`,
  0x1405c83a6: 0x217, 0x218, —, 0x219, 0x21a).
- While Genie is out the second deck's row 2 is "Genie" (0x25d, submenu 3); menu 3 lists his
  forms (`FUN_1403f3230` with `FUN_1403de370` and `FUN_1403de240`) aimed at him. A picked
  form goes the generic way (`FUN_1403a8b10` → `FUN_1403b5fa0`): trigger 11 of Genie's script
  with Sora, the command and its target. His script plays motion 241 and calls
  `genie_change_form` (10:9, `FUN_1401cf2d0`): the form command, the model hidden and shown
  again by the matching row (`FUN_1403cc9b0` / `FUN_1403cd810`, which take no part) and the
  form number kept for the next Genie (`FUN_1403ddfb0`). Each form has its limit command
  (0x21e, 0x220, 0x222, 0x224), which his script offers on the second deck's Limit row.

### Round (c): Stitch

- kn::Stitch (`FUN_1401c6d00`, think `FUN_1401ca240`) lives in camera space: a screen point
  (x right, y down, depth 1024) and a heading, turned each frame into the matrix the renderer
  draws him with (`FUN_1401cad30`, scale 0.503644, handed over by `FUN_1403cbfc0`); his object
  stays in physics mode 3, which never moves it. The think wraps the point at ±400 / ±320
  (by 800 / 640), runs the action table (0x140724340: stand `FUN_1401c7f40`, cross the screen
  `FUN_1401c8950` at most 15 a frame turning π/16 a frame, the HP special `FUN_1401ca6e0`, the MP
  special `FUN_1401c8100`, three limit commands) and, in battle after more than 5 actions, half
  the time runs a special directly (the HP one when Sora's HP part is not above his MP part);
  while Sora is hurt, his class's +0xe8 runs every 600 frames.
- The HP special (Sora under a quarter HP with an enemy up) walks to (−230, 90), turns, plays
  motion 0xcb and strikes every enemy once with ATKP 0x4ba (`FUN_1401caac0`); the MP special
  (Sora under three quarters MP) walks to (138, 106), plays 0xcc, fills Sora's MP
  (`FUN_1403c2040`) and starts effect 10 on him.
- Traps 10:1/10:2 set and read the screen point, 10:12 fills one of 24 effect slots
  (`FUN_1401c9c10`) that the personality's pre-update starts, follows and ends
  (`FUN_1401c9080`), 10:13 the shot's muzzle effect (`FUN_1401c9920`), 10:16 ends a slot,
  10:32 a goal (`FUN_1401c75e0`: eight fixed points or a random one from the tables at
  0x140724538 / 0x140724568) with motion 0xa1.

## What the rewrite does

- `test_room::summon`: the start, wait, placement, spawn and leaving above; the summon is a
  `ScriptBrain` running the object's own script with its own model and motion set (the
  gallery's loader and pose sampler), on Sora's team and of the summon class.
- `player::drive::Gauge::enter_summon`; `second_deck` shows Dismiss and routes Summon-list
  picks and Dismiss; `enemy::sysobj` now steps the object fade (+0xa08) each frame as
  `FUN_1403cc730` does.
- `--summon-tour <dir>`: each of the four summons with a full gauge [tour setup], the first
  three dismissed from the deck, the last running out of time; between the deck swaps Sora
  attacks (Attack every 20 frames) so that the summons that answer him have something to
  answer.
- `enemy::friend`: the kn::Friend controller above (think, sensing, following, selection,
  support pass, the watch list, Sora's attack notice) and `enemy::traps::friend`: table 7's
  friend traps and 35 of table 10's. `enemy::World::held` keeps the object the host runs and
  an object whose `sysobj_appear` is building another reachable to searches and handle
  lookups, as they are in the executable's single list. The summon object carries its
  objentry id; direct strikes on the enemy's stand-in land like the summon's attacks.
- `enemy::sysobj` ports the leave-ground fall; `btlobj_target` keeps the enemy-class test.
- Round (e): `limit_scene::Scene::summon` runs a limit whose program is a participant's own
  object (the summon), lent the summon's world and object for each tick
  (`Scene::swap_parts`, `Scene::release`); the summon's main thread runs with it, its think and
  acts are held (`FUN_1401b15f0` with `FUN_1403d7280`). The summon host shows the script's
  offers on the deck, starts the limit on a decided offer and sends follow-ups (trigger 23);
  Sora stands where the limit has him. `data::limit_file::read_summon` reads a summon's file;
  the brain's env answers the motion-window test (chain, decider, flags; Sora's own windows
  open). The summon tour starts each summon's limit from the deck and presses Attack and the
  reaction button for its follow-ups.
- Round (d): `second_deck` row 2 and menu 3 for Genie, `summon` keeps the form number, picks
  it at the start and sends a picked form to Genie (trigger 11); 10:9 shows the model and
  reports the form. The summon tour picks Genie's first form from the deck.
- Round (f): `Sysobj::limit_flight` (mode 0x34) and `Sysobj::limit_hover` (mode 0x2a), run
  every physics step like the other modes; `World::hover_spec`; traps 6:40, 6:26, 7:21 and
  10:51; `Env::movement_spec` (PRTY 0x3e, read by the summon host) and `Env::camera_view` (the
  follow camera's view); the summon host passes the stick (`player_get_movement`).
- `enemy::friend::stitch` (round (c)): Stitch's state, think, actions and effect slots, traps
  10:1, 10:2, 10:12, 10:13, 10:16, 10:32; `Sysobj::camera_space` (mode 3). The summon host
  places him in front of the follow camera from his screen point and refills Sora's MP for
  the MP special.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `cargo build --workspace --locked`, release `--features skate3` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Unattended `--summon-tour` (private `runs/s5`) | exit 0; all four summons start, appear, leave and end; Peter Pan ends by time |
| Unattended `--summon-tour` round (g) (private `runs/s48`, `runs/s49`) | exit 0, no faults or unsupported traps; Sora plays the captured motions in all four limits (Chicken Little 253, Genie's 253 / 255, Stitch's 253, Peter Pan 253 / 255 / 256) and stands between them |
| Unattended `--summon-tour` round (f) (private `runs/s38`) | exit 0; no faults; the four limits start and end, 41 follow-ups; Peter Pan's script calls 6:40 once and Sora flies along his heading at the PRTY 0x3e run speed (about 11.6 a frame, no stick in the tour) until the limit ends; Chicken Little's 10:51 is not reached in the tour (NOT RUN) |
| Probe `limit_probe aladdin` (private `probe-aladdin.txt`) | runs to its end without faults; without Aladdin as a participant 6:26 / 7:21 are not reached (NOT RUN for the hover) |
| Unattended `--summon-tour` round (e) (private `runs/s35`) | exit 0; all four limits start from the deck (529, 542, 519, 468), 41 follow-ups taken (Genie 543, Stitch 527, Peter Pan 470/479) and each limit ends; Peter Pan's lacks 6:40 |
| Unattended `--summon-tour` round (d) (private `runs/s28`) | exit 0; the second deck's row 2 lists Genie's four forms (535–538), the first is picked, his trigger 11 runs `genie_change_form` and form 1 is kept |
| Unattended `--summon-tour` round (c) (private `runs/s26`) | exit 0; no faults or missing traps; Stitch goes to the top-left point his script asks for, stands, crosses the screen; the others as in `s23` |
| Unattended `--summon-tour` round (b) (private `runs/s23`) | exit 0; all four follow Sora; Chicken Little's fireworks strike the Shadow twice (ATKP 1261) and he falls back after `surprise`; Peter Pan starts `attack_small` on Sora's attacks; Genie waits on Sora's commands; Stitch stands (traps 10:1, 10:12, 10:32 missing) |

## Uncertainty

- [I] The first-time `appear` event is not played (the room takes the setting-Off path).
- [I] The summon is removed when its leaving fade ends: the removal's trigger in the kn::Friend
  update was not traced.
- [I] The summon level is 0 (no save is loaded); the team is Sora's (not traced in
  `FUN_1401b06e0`); the placement's wall line samples the room's wall push
  (`FUN_14040c2b0` not ported) and its last fallback is Sora's point.
- Summon limits not finished: Chicken Little's first-person part (he is not drawn in his
  limit); the limit's cost in summon mode (`FUN_1403a9cf0`); the follow-ups' motion windows are
  not tested before they are sent [I].
- [I] A captured slot's variant is chosen as in battle with the Keyblade out; its second
  weapon part ("0012") is not loaded; a capture of another victim than Sora is not drawn.
- [I] Mode 0x34's forward vector (`FUN_1403b56e0`, +0x680 or the +0x60/+0x68 matrix terms
  while `FUN_1403da520` holds) is read as the heading's; its exit's PRTY record 1 is read as
  the turn rate the object had before. Mode 0x2a copies `DAT_142b59470` when it is entered
  (the executable reads it each step); a mode change gives the turn rate back.
- [I] 10:51: the projection's scale is read as the follow camera's focal length on the
  640-wide screen (320 / tan(fov / 2)); the iterator's skips (`FUN_1401b51a0`,
  `FUN_1403bd420`, `FUN_1403ba4a0`) are read as the nearest-target search's alive test; the
  cleared record's kind is −1.
- [I] The Genie form list's colours (`FUN_1403b0f20` with his record) are 0; the target
  record handed to trigger 11 is 0; the form generator starts at 1 for each summon.
- Stitch's limit mode (traps 10:3/10:4 `FUN_1401c9d60` / `FUN_1401c7e00`, the commands
  0x20d–0x20f, the damage shots of 10:13 with attack 0x373 and the target cycling
  `FUN_1401c96c0`) is not ported; Genie's limit commands and forms and the summon finishers
  are not ported. The summon host does not draw script effects for any summon yet.
- [I] Stitch's camera-space matrix: the extra terms `FUN_1401cad30` writes into the second
  matrix (+0x6e4 −10, +0x704 1024) and the second block (+0x708, the point at 0x140724330)
  are not interpreted; he is placed at his scaled screen point in front of the follow camera
  (view x right, y down, z forward) and drawn at his model's own size. The motion root's
  heading is not subtracted. The special strikes land with Sora's stats; FUN_1403cf830's
  operands are taken as `attack_new`'s. His class's +0xe8 call (the regeneration) is reported,
  not carried out [U]. The limit test (`FUN_1403d8ad0`) is the running limit's command.
- [I] One Sora attack notice per Attack/AirAttack action change (Sora's +0x180 is not kept),
  aimed at the room's enemy while one is there; the notice ages once per frame after it is
  noted.
- [I] The friend watch-list sweep runs at the start of the friend's own update; the
  leave-ground fall takes collision as enabled (`FUN_1403ba940`), "on the floor" as within one
  unit of it, and sends no script event 0xe; the fall's arc is not frozen by the range that
  is still playing.
- [I] Direct strikes land at the stand-in's position with Sora's attack stats (the summon's
  sheet is not traced); hit callbacks of strikes are run as for volume hits.

## Verdict

PARTIAL. Summons come, last their time and leave as the executable sequences them; Chicken
Little, Peter Pan and Genie run their scripts under the ported kn::Friend controller and follow
Sora; Chicken Little and Peter Pan attack; Stitch roams the screen and runs his specials;
Genie's forms are chosen from the deck; every summon's limit starts from the deck, takes its
follow-ups and ends; Peter Pan's limit flies Sora, and Chicken Little's limit can pick the
target on screen; Sora plays the motions the summons capture for him. Chicken Little's
first-person part remains.

## Next task

Chicken Little's first-person part of his limit.
