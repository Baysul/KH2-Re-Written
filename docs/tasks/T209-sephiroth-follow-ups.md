# T209 — Sephiroth follow-ups: his late attacks, the clear overlays, voice and physics fixes

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do"), relayed with the list T206 left open. Gate: enemy AI, combat and
scripts (P19/P20 work; nothing here is inferred as a gate pass). Prerequisites: T206 (boss
framework and Sephiroth), T207 (summons and party), T140 (game over's blur). Build identity:
branch `feature/boss-sephiroth-2` cut from `main` at `6de4a1f`; package 0.1.2, Rust 1.99.0
Windows MSVC. Source: the selected Steam build 15194255, validated at every run; the install was
only read. (T208 is taken twice by open work, the debug menu and the reaction-command manager;
this record is T209.)

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/boss-01/` (plain
`analyzeHeadless` decompiles `out/e1`..`e5`, `f1`..`f3`, reference scan `out/f2-refs.txt`;
project copy `ghidra-kh2-sep1`, read only), `research/sephiroth-01/` (the script listing,
`rc-01` and `host-01` specs, `dof-01`), and earlier decompiles reused from
`research/enemy-ai-20261016/` and `research/playable-room-20261008/jump-01/`. Unattended runs:
the private `kh2-boss-out/t8`..`t17` (logs and captures). No Ghidra MCP was used.

## What the executable does (and what is now translated)

- **Overlay events run at the event rate.** The clear overlays are event scripts (camera type
  2) and step at 30 frames a second: half a frame per 60 Hz tick, times the game speed. T206
  stepped them a whole frame per tick.
- **Overlay camera.** FUN_1402cc4b0 plays a camera-type-2 event with callback FUN_1403ac630
  (also run by the event's Tag records, opcode 0x51), which sets the base the event camera sits
  on (FUN_140406bc0 / FUN_140406a90). For the clear
  overlay (anchor object 0x236, Sora) states 3 then 7: Sora's matrix is copied once, then its
  translation follows his position (FUN_1403b5b40) every frame. For the boss overlay states 4
  then 8: the dead boss's matrix (DAT_142ae7d30 +0x40), then the position of his death effect
  (DAT_142ae7d38, FUN_1402be970). The event camera's eye and target are local to that base
  (FUN_1402d37b0); `SeqCameraCollision` (opcode 0x46) keeps the camera out of the map.
- **Radial blur.** The overlays' `SeqBlur` switches feed the same feedback pass as cutscenes
  (FUN_14015f250): the previous picture, zoomed (514/512, 418/416) about the centre, over the
  scene at alpha/128, a new picture taken once per event frame. Game over's death event uses
  the same pass, so its blur now zooms too.
- **Voice priority.** An object's YS::VOICE_EE (FUN_1403ce580) takes a request when nothing
  waits or plays, or when its priority is at least the held one; its update (FUN_1403ce260)
  stops the line playing (FUN_1401de500) and starts the waiting one, or drops a line that has
  finished.
- **Script attacks keep their hit identity.** An attack object keeps its id until the script
  frees it (destruction clears its seen bit on every record). The room retired a script
  attack's id every tick, so its Interval never applied: the grab's flurry (ATKP 1354,
  interval 4) hit Sora every tick for 50 ticks instead of every fourth.
- **Landing.** FUN_1403cb940 plays the land motion and queues idle behind it
  (FUN_1403c8310(motion 0)). Without the queue, Sephiroth's land motion (30 ticks, no loop, a
  no-control range from frame 0 with an open end) held its no-control flag after it ended, and
  his hop act (`move_hop`, which waits in a no-control loop) never finished.
- **A blow's landing** looks for the floor from above the step's start, as the hop's does. A
  finisher's blow that started a fraction under the floor never found it, and he fell through
  the map.
- **The grab's release on Sora.** The hook thread on Sora (`obj_fly_to_jump`, then
  `set_fall_motion(4)`, `set_land_motion(5)`) now starts his fall from where he is; motions 4
  and 5 are his own fall and land.
- **Vacuum on the party.** FUN_1403f9130 pulls every object by its objentry type and weight;
  Donald and Goofy now get the pull (as one more slide against the walls after their step).
- **Light tint on the fog.** FUN_1401626f0 also rescales the area's fog colour by the tint
  (bytes, `>> 7`); the room now updates every fogged material.

Audits:

- **His sounds and effects.** Every sound he requested in the runs resolved and played: his
  motion sounds 363040–363057, the victim sounds 363000–363003 on Sora, voice lines 5243024,
  5243029 and 5243032, and clear sound 23. Every effect he started was found (hit sparks, his
  own `.a.us` elements, the drive-style bound effects on bone 161). No script trap was reported
  unsupported.
- **His helper object.** `sysobj_appear(2322)` (B_EX360) is Shadow Flare's orb carrier: a
  thread per orb (words 6125, 6201 → 6334) moves it with the orb's effect, and the orb turns
  back on him when its attack (1170) is reflected by a Guard or when the carrier's OBJ_DAMAGE
  marks unit argument 1 (L6686). Guard reflection was already in; Sora's volumes now also hit
  the carriers' HIT spheres (read by the room's spawner from `obj/B_EX360.mdlx`).
- **The area's own event entry.** hb01 under programs 75 has one: signal 16 → event 602 and
  the jump; the mission clear sends it (T206).
- **Depth of field in event 601.** `SeqFocus` (opcode 0x18, FUN_1402da870) turns the pass on
  (FUN_140172100: a GS depth value, at least 15000, and a mode byte) or off (FUN_1401720f0).
  The pass (FUN_14018a9e0) redraws the frame as shifted copies at alpha 0x30/0x60 (0x20 in one
  mode) with a depth test at that GS depth, so only far pixels blur. Not translated: the
  mapping from the GS depth value to the PC port's depth buffer is not traced.
- **Lock-on and Scan.** No boss-specific rule found beyond T170's (the reticle group follows
  the HP pool; Scan shows 15 bars for 3000 HP). KH2 shows no damage numbers.

## Steps

1. Long unattended runs forced into his hard (HP 2000) and super-hard (HP 850) phases.
2. Room: overlay frame rate, camera on its anchor, `SeqBlur` with zoom; voice slots; script
   attack ids; party vacuum; fog tint; hook-thread fall on Sora; tour screenshots on demand
   (`KH2RW_SEPH_TOUR_SHOTS`), `KH2RW_SEPH_TOUR_LOW=<tick>[:<hp>]`, airborne state in the log.
3. Enemy crate: the land queue, the blow's floor probe, `Sysobj::falling`, made objects' HIT
   spheres. Room: a script object's OBJ circle kept out of the walls; Sora's volumes against
   the objects a script made.
4. Unit tests for the voice slot, the land queue and the blow landing.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and unattended runs:

```
KH2RW room <Steam-root> <manifest> <hb01-programs-75 config> --sephiroth-tour <new dir> --log <file>
  KH2RW_SEPH_TOUR_END=<ticks>  KH2RW_SEPH_TOUR_LOW=<tick>[:<hp>]  KH2RW_SEPH_TOUR_LOG=<every>
  KH2RW_SEPH_TOUR_SHOTS=<tick>,<tick>,...
```

The tour is a test setup (Sora at 9999 HP, attacking every 10 ticks, pressing Triangle on every
other offer; the boss's HP set once), not a game rule.

## Uncertainty

- Nothing here was compared with a capture of the installed game.
- The overlay camera's frame: FUN_1402d37b0 multiplies the event camera (after FUN_1402d3d40)
  by the base matrix; in the room's scene frame the channels are read as they are and turned
  by the anchor's yaw. The eye heights in `t16` (300–600 up) confirm the y sign; the z sense
  and the roll's sign are [I]. The boss overlay follows his TARGET shape centre, where his death
  effect is bound (bone 161), not the effect's own position [I]; the anchors' matrices are taken
  as rotation only. The camera collision (opcode 0x46) is taken to be the follow camera's test
  from the target (FUN_140164940) [I].
- The radial blur's history is the scene without the HUD, as for the Drive afterimage [I].
- The voice slot's starting priority is taken as 0 [I]; the level 0x1800 is not applied.
- The party's vacuum pull is applied after the member's own step [I].
- The tint does not reach the map effects or the calls FUN_1401714d0 / FUN_14016f2b0, and a
  tinted neutral fog does not turn fog on.
- The Block marker and the HUD burst are left to the reaction-command manager's branch
  (`claude/project-thread-kyocfc`), which draws the marker as a dk::LockOn of group 0x3a.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`),
  workspace build: PASS. Release helper: see the pull request.
- New unit tests: the voice slot's priority and replacement, the land queue, a blow that
  starts just under the floor.
- Runs against the install (programs 75, tour as above):
  - `t8` (before the fixes): the grab ran (slot 254, camera watch, white flash), but ATKP 1354
    hit Sora every tick for 50 ticks.
  - `t9` (HP 2000): his first hop never ended (motion 5 held its no-control flag); `t10` (HP
    850): Sora's finisher blew him through the floor; `t13`: the same after the floor probe
    fix, as the blow carried him off the arena (no wall test for script objects).
  - `t12` (HP 2000, fixes in): Heartless Angel at tick 1812: Sora's HP to 1 (−9762) and his MP
    into a charge at 2062. The grab at 1380 released Sora into his fall (1453); the flurry hit
    13 times, every 4 ticks (1452–1500). The run then reached the game over (Sora at 1 HP took
    a hit), which is the game's rule; the tour now refills him.
  - `t14` (HP 2000, 9000 ticks): five Heartless Angels, 23 hops, grab and release, no hang.
    His OBJ radius (70) keeps him inside the walls.
  - `t15` (HP 850): Meteor three times (from tick 3210), Shadow Flare S and L, Power Up.
  - `t16` (HP 30): death at 3542, mission clear at 3729; the boss overlay's camera looks at his
    upper body from his side (eye ~600 away, 300–500 up), the clear overlay's looks down on Sora
    from above; `SeqBlur` 60/128 then 80/128 with the zoom. Captures 60, 63 and 64 show both
    shots and the blur.
  - `t17` (HP 850): Shadow Flare L made ten B_EX360 carriers, each with one HIT sphere; no tour
    swing struck one, so the struck-orb path was not seen.
- Voice: lines 24 (tick ~38) and 29 (at the hard phase) were taken and played on the next
  update.

Not seen or not right yet:

- **The script's space is mirrored.** The object world runs in the room's scene frame, but
  `native()` flips only y, so script space is the engine mirrored in z. Live positions stay
  consistent; engine constants do not: Meteor's spot (engine −750, −600, 2250) lands at scene
  z +2250 instead of −2250, and the arena centre W32 and fixed turns are mirrored too. This
  touches every script-driven object (the summons and limit scenes as well) and is the next task.
- After Heartless Angel he ends at y 590 and the pushed Split Flash starts there; Block is
  offered but the counter (slot 253) does not follow. Ground mode does not bring an object left
  in the air down (FUN_1403b9090's other branches are not translated).
- The B_EX360 carriers are still listed after their orbs end (the script's `obj_leave` path
  depends on `effect_is_active` going false), and their struck path was not exercised.
- Opcode 0x41 (DAT_142a11400 bit 5 set/clear) is not carried out; opcode 0x51 (Tag) values
  0, 100, 101 and 200 are not run; the mission-start overlay is not started.

## Verdict and next task

PARTIAL. Heartless Angel, Meteor and the grab now run from his script against the install; the
grab's hit rate, the hop, the blow and the walls are fixed from the exe's rules; the clear
overlays run at the event rate with their camera and blur; the voice rule, the party's vacuum
pull and the fog tint are in. Nothing was compared with the original game, so no behaviour is
claimed 1:1 by observation.

Next: (1) the script space: `native()` as the engine's frame (y and z), with yaw and turns
converted at the trap boundary, then the summons and limit scenes rechecked; (2) ground mode
bringing an airborne object down after Heartless Angel; (3) depth of field in event 601
(the GS depth mapping); (4) a side-by-side capture against the installed game.
