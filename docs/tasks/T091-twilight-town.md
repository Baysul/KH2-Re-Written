# T091 — Twilight Town areas, exits and townspeople

Owner Claude, 2026-10-10. Gate P16/P21/P24 (rooms, streaming, actors). Starts from T090 and the movement branch (World::collide).

## Source

Private evidence (research root `playable-room-20261008/tt-world-01/`): `area-01/area-spec.md` (area load, ARD scripts, spawn sets, story programs), `exit-01/exit-spec.md` (exit zones, iris, place names), `collide-01/collide-spec.md` (map collision), `npc-01/npc-spec.md` (RTN townspeople), `draw-01/draw-spec.md` (map draw groups, texture regions, sky, map objects). Ghidra projects `ghidra-kh2-t1..t5`. Build 15194255.

## Implementation

- `package_reader::named`: MD5 of a path names a HED record, so areas load as `map/tt08.map`, `ard/us/tt08.ard`, `obj/<model>.mdlx`.
- `package_reader::ard`: script blocks, commands, spawn sets (FUN_1403a23e0, FUN_1403a24c0, FUN_1403fdce0).
- `area_sim`: 00progress flag scripts (FUN_14039afc0), the ARD command interpreter (FUN_1403a24c0), spawn activation with the entrance filter (FUN_1403ff550), exit zones (FUN_140420de0, FUN_140421180/250, FUN_1403ffb40).
- `field_collision`: the sphere sweep (FUN_1401752e0, FUN_140174640, FUN_140175190), polygon gather (FUN_1401741c0), segment ray (FUN_140173b30) and floor probe (FUN_140172840, FUN_140172640). Sora's body is the P_EX100 type-23 entry 0: radius 90, height 90, centre 115 above the feet.
- `kh2fm tt` (launcher: "Twilight Town"): `area.rs`, `npcs.rs`, `test_room.rs`. Station Plaza (tt08, entrance 0), story flags 0x800..0x895. Exits play the 'mapj' iris and load the next area at its entrance. Maps draw by DOCT group under the area's mask (zone switches included); texture environments use their CLAMP region, address modes and linear filter; opaque chunks alpha-test at 31.5/255; map objects (BOB triples) are placed from the 'out' table and play their motions (a train crosses the Station Plaza sky); sky models scroll by UVSC. Entering an area shows its place name (msg/us/place.bin) and the exit guide shows the destination name. Each area's BGM pair comes from the 03system 'arif' table (ARD op 0x10 overrides) and changes when the id differs from the previous area (id 0 silences, 1 s fade). Townspeople (objentry type 15) load by name, walk their Hermite paths with the walk/turn motions (rtn_default path_move) and talk through the field talk window with their libretto script.
- Area environment (type 14, draw-01 §8.1): the fog is `f = clamp((far - w)/(far - near), min/255, max/255)` blended in gamma space by a `StandardMaterial` extension (`crates/windows_shell/src/fog.rs`, `fog.wgsl`); sky layer 0 is never fogged; the record's clear colour becomes the scene clear colour.
- 'dsa' story blocks (exit-01 §2.6): progress ops 2-5 set the per-destination message word; a blocked destination refuses the jump and shows its disable-reason text for 6 s. `kh2fm tt` opens every destination (`Progress::unblock_all`) and, when the story point's map program places no player at an entrance, tries the area's other map programs in file order.
- NPC-NPC meets (type 5, FUN_1404179e0): the first overlapping townsperson with matching zone-0 records starts both on the record's motion, facing each other, for 360..720 ticks.
- Head look-at (look-02): one bone turned by the anb evaluator's look branch (`model_pose::apply_look`), yaw/pitch limits and 5/3 degrees per tick from the type-20 shape, weight ramp 20 ticks, active during the look act and the encounter's face/hold phases, only while idling or turning.
- TEXA texture animations (draw-02 §1): sprite sheets swap into the environment's texture by their frame script, with the exe's 1-texel border rule.
- `KH2FM_TT_STORY` (last story flag, hex) and `KH2FM_TT_AREA` (`area[:entrance]`) choose the story point and starting area for `kh2fm tt`.

## Checks

PASS: fmt, clippy -D warnings, workspace tests (incl. TEXA frame state machine and upload border, look-at math, meets, 'dsa' ops), debug and release builds. All 42 areas load at entrance 0 with the player placed (release exe sweep). Unattended runs: tt08 → tt06 exit and arrival; a referee in tt07 spawns on zone entry, walks, and starts a talk with its libretto line and window; TEXA workers start in tt01 (2), tt21 (6) and tt30 (7).
NOT RUN: capture comparison against the original for fog, TEXA sprite nibble order (low nibble first is [I]), head look-at orientation (an encounter with a head-tracking model was reached but the capture was a talk frame).

## Uncertainty

- NPC routes follow the Hermite curve but the projection and look-ahead use a sampled search with a 33-step refinement rather than FUN_1405513a0/FUN_1405510d0 line by line.
- Not ported, traced in private specs (tt-world-01/): glow post-effect (draw-03: scene colour where stencil 0xFF, 4x downscale, 6x two-pass blur, additive composite; enabled per area by arif flag bit 4), BOB LOD and part fade (draw-03 §2, TT values 1000/1200/2000/2200), map PAX effects (pax-01: sun and lens flare, dust, vertex swarms; same container as object PAX but needs the effect-program VM and the WA_t occluders), NPC push (look-02 §2: 5/6 to Sora, 1/6 to the NPC, one-shot displacement), pop-in/out alpha fade (10 ticks linear, FUN_1403b7810), rtn_sit/rtn_lm/skater variants (rtn-02: every TT rtn_sit/rtn_lm entity but the Zipper is fixed and idles; skaters run the default code), subtractive blends (none exist in TT), alpha-pass batch order (draw-02 §3: ordered by leader chunk then sorted members).
- Not applied: story-state selection from a save (fixed "first visit" point or the env vars), cutscene events and signals (type-22 events, EventEntry signals) and area enemies/battles.
- Walk speed is the motion root record translation over its length in ticks; the footer reading is [I].
- Story state is the chosen "first visit" point (flags 0x800..0x895 ascending) [I].

## Verdict

PROGRESS. Next: the glow pass, NPC push and pop fade, map effects (needs the effect-program VM), cutscene events.
