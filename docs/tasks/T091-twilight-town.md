# T091 — Twilight Town areas, exits and townspeople

Owner Claude, 2026-10-10. Gate P16/P21/P24 (rooms, streaming, actors). Starts from T090 and the movement branch (World::collide).

## Source

Private evidence (research root `playable-room-20261008/tt-world-01/`): `area-01/area-spec.md` (area load, ARD scripts, spawn sets, story programs), `exit-01/exit-spec.md` (exit zones, iris, place names), `collide-01/collide-spec.md` (map collision), `npc-01/npc-spec.md` (RTN townspeople), `draw-01/draw-spec.md` (map draw groups, texture regions, sky, map objects). Ghidra projects `ghidra-kh2-t1..t5`. Build 15194255.

## Implementation

- `package_reader::named`: MD5 of a path names a HED record, so areas load as `map/tt08.map`, `ard/us/tt08.ard`, `obj/<model>.mdlx`.
- `package_reader::ard`: script blocks, commands, spawn sets (FUN_1403a23e0, FUN_1403a24c0, FUN_1403fdce0).
- `area_sim`: 00progress flag scripts (FUN_14039afc0), the ARD command interpreter (FUN_1403a24c0), spawn activation with the entrance filter (FUN_1403ff550), exit zones (FUN_140420de0, FUN_140421180/250, FUN_1403ffb40).
- `field_collision`: the sphere sweep (FUN_1401752e0, FUN_140174640, FUN_140175190), polygon gather (FUN_1401741c0), segment ray (FUN_140173b30) and floor probe (FUN_140172840, FUN_140172640). Sora's body is the P_EX100 type-23 entry 0: radius 90, height 90, centre 115 above the feet.
- `kh2fm tt` (launcher: "Twilight Town"): `area.rs`, `npcs.rs`, `test_room.rs`. Station Plaza (tt08, entrance 0), story flags 0x800..0x895. Exits play the 'mapj' iris and load the next area at its entrance. Maps draw by DOCT group under the area's mask (zone switches included); texture environments use their CLAMP region, address modes and linear filter; opaque chunks alpha-test at 31.5/255; map objects (BOB triples) are placed from the 'out' table and play their motions (a train crosses the Station Plaza sky). Townspeople (objentry type 15) load by name, walk their Hermite paths with the walk/turn motions (rtn_default path_move) and talk through the field talk window with their libretto script.

## Checks

PASS: fmt, clippy -D warnings, workspace tests, build. Unattended runs: tt08 → tt06 exit and arrival; a referee in tt07 spawns on zone entry, walks, and starts a talk with its libretto line and window.

## Uncertainty

- NPC routes follow the Hermite curve but the projection and look-ahead use a sampled search with a 33-step refinement rather than FUN_1405513a0/FUN_1405510d0 line by line.
- Not ported: NPC interrupts (zone actions, meets, encounter), look-at, NPC push, rtn_sit and other AI programs, pop-in alpha fade, place-name display, destination-name guide, 'dsa' blocks (never blocked here).
- Not applied: fog and the area clear colour, the glow post-effect, per-object LOD and fade distances, batch ordering of the alpha pass, subtractive blends, UV scroll (sky UVSC) and TEXA frame swaps.
- Walk speed is the motion root record translation over its length in ticks; the footer reading is [I].
- Story state is the chosen "first visit" point (flags 0x800..0x895 ascending) [I].

## Verdict

PROGRESS. Next: fog and clear colour, UV scroll, then NPC meetings and look-at, the remaining rtn programs, and the other worlds' exits.
