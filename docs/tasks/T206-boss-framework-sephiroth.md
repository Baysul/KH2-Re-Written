# T206 — Boss framework, and Sephiroth on it

Owner Claude, 2026-10-23, requested by burba in the project chat ("I want to start working on
boss framework, so the first boss i want to complete recreate is sephiroth 1:1 using ghidra with
all attacks reactions commands vfx sfx everything"). Gate: enemy AI, combat and scripts
(P19/P20 work; nothing here is inferred as a gate pass). Prerequisites: T200 (the Sephiroth
fight as it ran from the install), T125/T133/T190 (object scripts and services), T170 (lock-on
and Scan), T140 (game over). Build identity: branch `feature/boss-framework-sephiroth` cut from
`main` at `1f3b241` with T200's four commits carried over (PR #115 is replaced by this one);
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build 15194255, validated at
every run; the install was only read.

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/sephiroth-01/`
(the T200 research and three specs written for it: `host-01` stats, boss class, mission;
`rc-01` the reaction command; `revenge-01` the revenge gauge, boss damage and death) and
`research/boss-01/` (this task's headless Ghidra decompiles `out/d1`..`d9`, function lists and
operand scans; project copy `ghidra-kh2-sep1`, read only, plain `analyzeHeadless`). Unattended
runs: the private `kh2-boss-out/t1`..`t7` (logs and captures).

## What the executable does (and what is now translated)

Engine object rules, for every enemy-class object (enemy crate):

- **Class.** The object factory builds a BOSS for objentry type 3 (`+0x6c8 |= 0x400`, the
  state pattern `+0xd10 |= 0x8404`) and a ZAKO for type 4. Sephiroth (2230, `N_HB630`) is type 3.
- **Revenge gauge.** `+0xd48` starts at 0 and its limit `+0xd4c` at 100.0. Every hit adds its
  ATKP RevengeDamage (+0x2a) first (FUN_1403dbb90); the gauge falls by the frame time each tick
  while the act manager is active (FUN_1403dbbf0 / FUN_1403ca250); motion frame event 0x1c starts
  the act `revenge` by name when the gauge has reached the limit (FUN_14040dd70). Sephiroth's
  `revenge` act warps him away.
- **A hit, in order** (FUN_140410d60): pattern bit 13 ignores it; a dead or invulnerable object
  (FUN_1403d3570) takes nothing; the damage is applied (FUN_1403d3ba0) with the kill veto
  (FUN_1403d35f0: a boss's pool 0 cannot reach 0 unless the ATKP has Flags bit 5, so normal hits
  leave 1 HP) and the pattern-15 rule (a normal-reaction hit by anyone but the leader does at most
  1); the HP change may kill (death: the `dead` act, and for a boss element 0 of its effects and
  event signal 4 with its entry id, FUN_1403f8c60); the `patn` table rewrites the reaction by the
  active pattern bits (FUN_1403ee9b0); the script's OBJ_DAMAGE runs; the engine reaction
  (FUN_1403d2fe0) plays unless the script carried one out (`damage_exec_reaction`).
- **Sheet.** `btlobj_set_sheet(n)` binds ENMP row n (row 69 when absent) at its level, or the
  battle's when 0 (FUN_140411350 → FUN_1403c2240). Sephiroth binds row 140: HP 2000 % of the
  LVPM HP; the area's battle program sets level 80 → **HP 3000** (15 bars on the Scan gauge),
  damage cap 23, minimum 8, weakness physical/light/general 100, fire/blizzard/thunder/dark 0.
- **Movement request.** FUN_1403b4980 clears the request (+0x10/+0x1c) every tick; a script
  that keeps moving sets it again. (Without this, a warp's one-tick request kept moving him.)
- **Reaction commands an object offers.** REACTION collision shapes (kind 5: the argument is the
  command) are on when FUN_1403ba600 says so: script entries (traps 1:49..51), motion entries
  (ranges 6/7, cleared each tick), script group bits (1:52..54), motion group bits (ranges 4/5),
  then the table's default mask. Sephiroth's shape 18 (group 3, radius 3000, command 664
  "Block") is switched on by range 4 [3] of motion 204 (Split Flash) between ticks 19 and 49.
- **Script threads.** `magic_start_thread` and `obj_hook` start a thread of the calling program
  that shares its work area (FUN_1403c5ec0 / FUN_1403d76d0); they run every tick until their
  code returns.
- **OBJ_ATTACK.** When an attack reaches Sora, its owner's script gets trigger 15 with Sora and
  the attack, whether or not Sora could be hurt (FUN_1403d1730 → FUN_140410ce0).
- **Other services:** `obj_add_hp` (vt+0xe8), `obj_start_mpdrive` (MP to 0 and an MP charge of
  f × PRTY +0x1c), `obj_is_star` (Sora's invulnerability), the vacuum pull (FUN_1403f9130),
  script lights as a global map tint (FUN_1403f7040 → FUN_1401626f0), Sora's bones 0x4000/0x4001
  (FUN_1403b50f0), `camera_watch` (camera mode 3: the lock-on routine with look blend 1.0), the
  screen white-out/in (FUN_140156ce0/c90 with `(int)(x / 2.0)`), the object's voice
  (YS::VOICE_EE plays line `id` of its `voice/...` bank, FUN_1403dad90 → FUN_1401de230).

Sora's side (player crate):

- **The Triangle search** (FUN_1404081c0 → FUN_140407de0): REACTION shapes touching Sora's OBJ
  ellipsoid, the command record's battle flag and state lock, priority then score (distance,
  ×0.5 for the locked-on target, ×0.8 for the one shown), the facing gates and the
  no-control dimming (FUN_1404088f0), the form gate (FUN_1403b6a10) and the mode
  (FUN_140401d60). The command is taken only in an actionable window (FUN_1401b15f0), then the
  target's script gets trigger 11 (FUN_1403a8b10 → FUN_1403b5fa0).
- **Captured motions.** `obj_motion_capture` plays a slot of the captor's set on Sora (each
  slot holds a body and a Keyblade clip); Sora's kit loads Sephiroth's 252 (Block pose), 253
  (the counter) and 254 (the grab).

The mission (room, `boss.rs`): hb01's battle program registers mission 236 `HB01_MS601`
(`msn/us/HB01_MS601.bar`). Its record: flags 0x1b1 (bit 0 forces battle style 2), bonus 35,
clear sound 23. On the boss's death signal the mission script's trigger 10 runs
`trap_mission_dead_boss` (FUN_1404334d0): bonus 35 (+1 Drive bar), the clear sound, the won state
(DAT_142a11400 bit 1: commands locked, Sora unhurt), then the overlay `re_deadboss-0a` (game speed
0.1 at frames 63–76, a white flash at 0) and, four frames before its end, `re_deadboss2-0a`
(game speed 0.1 for frames 0–20, a white fade from frame 6 over 70); when both are over the
signal 0x10 starts event 602 (FUN_1403a35c0).

Music: hb01 under program 75 gives BGM 188 for field and battle; the manager plays slot 0 and
never switches (one track), and the mission's battle style 2 keeps the battle state fixed.

## Steps

1. Carried T200's four commits onto a branch from `main` (one conflict each in
   `enemy_battle.rs`, `test_room.rs` and the task index, resolved by keeping `main`'s code).
2. Enemy crate: boss class, revenge gauge, the hit in the exe's order, kill veto, `patn`, sheet
   binding, reaction groups and traps, side threads, OBJ_ATTACK, `obj_add_hp`,
   `obj_start_mpdrive`, `is_star`, vacuum pull, light tint, the per-tick movement request.
3. Data readers: ENMP pools and bonus, `patn`, the collision table's default mask, the command
   record's reaction fields, the objentry weight.
4. Player crate: the reaction-command search, captured motions, an outside push, MP charge
   start, camera mode 3.
5. Room: Sephiroth's sheet and the area's battle level; programs 75 for the fight; the Triangle
   slot and press; Sora playing captures; white flashes; camera watch; voice bank; spawner;
   vacuum; map tint; the mission record and clear; one-track music; `--sephiroth-tour`.

## Checks

See Results.

## Uncertainty

- Nothing here was compared with a capture of the installed game. All timings and numbers come
  from the exe and the install's data, not from watching the original.
- Event frames of the clear overlays are advanced by the game speed; the room steps its field in
  whole steps (a 0.1 speed is one step in ten), where the exe scales each step's time: the slow
  motion is choppier here.
- The overlays' cameras, radial blur and opcodes 0x41/0x46/0x51 are not run; the HUD reset and
  pause definitions of the clear are not done; the mission-start overlay is not started.
- The voice object's priority rule and level (0x1800) are not applied; every line plays.
- A boss captured motion started in the air ends on landing (the land motion replaces it), as
  the engine's physics would do [I].
- `obj_motion_capture_id` returns 0 once the captured clip has played; the exe keeps the last
  token (+0x184 is never cleared) [I/U]. Thread 770 of the grab ends then.
- The vacuum pulls Sora only, not Donald and Goofy. The light tint dims the map chunks only,
  not the fog colour or the map's effects.
- Scripts' hooks on Sora (`obj_fly`, `obj_fly_to_jump`, fall/land motions) act on Sora's
  stand-in, not on the room's Sora.
- The reaction-command target marker (effect 0x3a) and the HUD burst are not drawn.

## Results

Pending.

## Verdict and next task

Pending.
