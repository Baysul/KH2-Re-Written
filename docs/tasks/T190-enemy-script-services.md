# T190 — Enemy script services: searches, physics modes, attack callbacks, routes

Owner Claude, 2026-10-17 and 2026-10-18, requested by burba in the project thread (GitHub
issue #61, which merges #60 and #64). Gate: enemy AI (not passed). Prerequisites: T133, T125.
Build identity: branch `claude/project-thread-dgxtsf` cut from `main` at `554d3bf`,
package 0.1.2, pinned Rust 1.99.0 Windows MSVC. Source identity: installed Steam build
15194255, read only; no writer or mod manager was launched against it. The routines were
read from a private copy of the analysed Ghidra project and the 797 object scripts
extracted to private storage; nothing from them is in Git. Each translated routine names
its executable address in a comment.

## Steps and evidence

1. `source/game/enemy/examples/script_sweep.rs`: a research example that runs extracted
   object scripts in a synthetic arena (flat floor, every motion 30 ticks, Sora standing
   still). A script that searches for an object it does not find gets a partner object of
   that description on the next round (a real script when an entry table says which), and
   `sysobj_appear` makes real objects, so scripts run in a world with the objects they
   expect. It also lists the trap slots that have a translation (`--implemented`).
2. Object list searches over the world's objects: by entry id (Sora is 0x236), spawn
   serial, part id and unit argument; a miss binds nothing. `sysobj_appear` asks the
   host's `Spawner`. `obj_each_all` and `enemy_each` walk the list. Engine handles read as
   the AI object scripts copy, target-record copies and Sora's lock-on record can be read,
   and words a script writes through an object handle read back (the spec's maximum fall
   speed, for one).
3. Physics modes: hover (mode 6) is mode 5's update with mode flags 0x100; hop (mode 9)
   with the executable's integration (vertical speed updated before it is applied, capped
   by the spec word at +0x138). The take-off event now maps pending mode 2 to the hop.
   Arming a jump or a hop resets the fall and land motions to (none, 5); a jump or hop
   plays its fall motion five ticks before the apex and its land motion on touching down,
   which the earlier translation stored but never played.
4. Attack objects: hit and reflect callbacks (`Object::attack_struck`,
   `Object::attack_reflected`) with the arguments the executable passes; team mask,
   reflect marking and `attack_strike` reach Sora through the test room's hit path; the
   map flags and `hit_bg_pos` are set from the room's floor and walls; the timed-shape
   radius is the executable's quarter sine from the `attack_set_time` pair to the attack's
   own pair, and the attack is freed when its time is up or its bound effect ends.
   `attack_get_reflect_vector`, `attack_get_reflecter`, the hit mark vectors and the
   reaction offset are translated.
5. Shouts: `act_shout` queues; the frame's shout job gives each object the shout, which
   calls the timeout routine of its act of that name (immediately, or after a random share
   of the delay drawn from the engine's generator). The shouter is a receiver too.
6. Target searcher (init, add, reset, clear, search, search one object), lights (create,
   flags, colour, fades, fog), vacuums, the global status bits scripts set, enemy units,
   carried objects (`obj_attach`, `obj_detach`), model hide/show and texture animation,
   the texture fade, `sysobj_moveto`, `obj_move_to_space`, the vector cross and dot
   products, the object state queries and setters in `traps/query.rs`.
7. Requests to subsystems this crate does not own (camera, screen, mission HUD, messages,
   progress flags, party, prizes, dice and cards, friends) become `Event::Service` with the
   trap's operands decoded. Their values (pad, mission timer, count, gauge, score, enemies
   left in a unit, the camera's eye and target, bone matrices) come from `Env` methods that
   default to 0 or none. This is the engine side of the call.
8. 2026-10-18, routes (`source/game/enemy/src/route.rs`, `rtn.rs`, `traps/route.rs`): the
   executable's route follower (FUN_14054df70 to FUN_140551850: Hermite tangents and
   points, the nearest-point search with its halving refinement, the look-ahead
   FUN_14054f9d0 with its turn-round and wrap rules, active paths that follow another
   object, building a route from a spawn point's walk path, reverse, leave-way, walk by
   length), the routine action state the `5:*` traps read (get/end/is_end action, action
   direction, partner, head position), `obj_set_path`, `obj_can_look`, and the event path
   walker (`eh22_path_*`, whose point list and arrival belong to the host).
9. 2026-10-18, `bg_get_random_pos` and `_air` (FUN_1403f3fa0, FUN_1403f4080: the ring
   search around a centre with the engine's generator and its lap counter),
   `strike_raid_calc_xyzrot` (FUN_14053ef90 with the Euler angles of FUN_1401ab290), the
   battle target table and `target_set_before_player` (FUN_14040c870, FUN_14040c9d0) used
   by target search kinds 0, 1, 3 and 5, `obj_set_pos_trans`, `obj_dead_mark`,
   `obj_get_bg_press`, `obj_play_se` (an `Event::Sound`), the head look
   (`obj_look_start/end`), `obj_motion_capture` with its token and the clip-length timer
   that ends it, `obj_motion_hook`, `obj_hook`, and the light-cycle steering
   (FUN_140421940).
10. Test room: the script brain reports its sound effects (played through the room's sound
    queue), voice lines, and the names of the requests nothing here plays, once each, to
    the log (`enemy_script_voice`, `enemy_script_service`, `enemy_script_unsupported`).
    A record naming an object no longer overflows when its handle is read (a debug-build
    panic the battle target table exposed).
11. Synthetic tests for each of the above (`object/tests.rs`, `sysobj/tests.rs`,
    `route/tests.rs`, `traps/*/tests.rs`, `attack/tests.rs`). They use generated bytes
    only.

## Checks

- Arena sweep of the 234 installed enemy and boss scripts, 1500 ticks each: before this
  task 196 ran without a fault or an unimplemented trap and 11 stopped on a script fault;
  now 234 of 234 clean (private tool, final run PASS; main scripts only, partner scripts
  can still fault, see Uncertainty). A run without a fault has not been shown to match
  the game.
- Static coverage (trap slots the 234 scripts reach, by a linear decode of the extracted
  scripts): 188 untranslated before, 0 now (the decoder reports three slots that are not
  calls: 4:0, 4:1 and one stray value). Several are approximations or host hand-overs,
  listed under Uncertainty and Not done; "translated" does not mean "played".
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked`, `cargo build --workspace --locked`: see the pull
  request for the results of the final run.
- Combat tour with `KH2RW_ENEMY_AI=script` (debug build): see the pull request.
- Comparison against the installed game for any enemy: NOT RUN.
- `docs/progress/coverage.json` and `python tools/progress.py`: NOT RUN; no reviewed
  evidence changed coverage.

## Uncertainty

- The attack map test is the room's collision applied to the attack sphere; the
  executable's own routine for attack objects was not located.
- Target shapes are one sphere at the object's origin unless the host's `Env` says
  otherwise; the candidate test of the searcher is the kind's class bit plus "alive".
  Target search kinds 3 and 5 (nearest enemy-class target) are translated from
  FUN_1403be7f0 with that stand-in; kinds 4 and 6 (ranges and angles from the object's
  own parameters, FUN_1403bea30 / FUN_1403be4b0) are not and answer Sora.
- `obj_hitback` keeps only the "no knock" gate of the executable's test.
- `obj_is_stop` models stop requests only; `obj_search_by_glance` measures from Sora
  instead of the camera's look-at point.
- Hop landing uses the jump mode's floor test; the executable's contact routine is
  shared with the other modes and was not read in full.
- The arena gives partner objects only the searches the main script made; partner scripts
  (for example B_EX180 and B_EX260 as each other's partner, or B_LK120's copies) still
  fault there.
- Class bits (zako, summon, Sora, enemy, routine, area paths) are host data; `ENEMY` is
  the default.
- Routes: the decompiler dropped three scalar arguments of FUN_14054ef80 (read as the
  direction components, which gives the projection), the argument of the random start
  position (read as the segment count), the first distance of FUN_14054f9d0's "no
  progress" test (read as the old curve point's), and the value FUN_1405513a0 returns
  (read as the best distance). The routine update that raises routine actions
  (FUN_1404179e0: zones and meetings) is not run by this crate; `main/src/npcs.rs` still
  holds its own, approximate route and does not use this follower.
- `bg_get_random_pos`: the collision mask of the map line check is not used; the test
  room's `line_hit` answers vertical segments only, so the clear-line test passes there.
- The light-cycle speed parameter (DAT_142af7bc8 + 4) and the mission gauge widgets are
  host data (`Env::lightcycle_speed`, `Env::mission_gauge_ratio`).
- The motion capture token is a count; the executable leaves its value in +0x184 by
  FUN_1403c64a0. A captured motion ends after the length of the captor's clip.

## Not done

- Playing the requests the engine hands on: camera shots, screen fades, the mission HUD,
  message windows, party and friend commands, prize tables, voices, magic and hook threads,
  captured motions on Sora, texture animation drawing, vacuum pulls. The test room has no
  subsystem for most of these and the three enemies it supports (Soldier, Shadow, Dusk)
  never call them, so nothing in the tour exercises them.
- A test-room `Spawner` and a world shared by the actors. No script of the supported
  enemies calls `sysobj_appear` or searches for another object (checked on the
  disassembly), so a spawner would have no caller; the engine side and the sweep's
  spawner are ready for it.
- Driving the routine action state (zones, meetings) and the NPC walker from this
  follower; the host's own NPC code stays as it is.
- The physics modes other than ground, jump, hop, fly, flyjump, steer, hover and blow,
  and the stop mechanism's effect on motion.
- Longer runs with a moving player, real motion data and damage and fall triggers.

## Verdict

PARTIAL. Task and build checks PASS (above); the combat tour result is in the pull
request. The enemy AI gate is NOT RUN: no enemy has been compared with the installed game.

## Next task

A shared multi-object world for the test room with a `Spawner` over the object table when
a boss with partners is admitted, the routine update for the route users, then controlled
installed-game comparisons for Dusk and Soldier.
