# T211 — Script vectors in the engine's frame (the mirrored script world)

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do"), as the first open item T209 left. Gate: enemy AI, combat and
scripts (P19/P20 work; nothing here is inferred as a gate pass). Prerequisites: T206/T209
(Sephiroth), T204/T207 (summons and limits), T125/T133/T190 (object scripts). Build identity:
branch `feature/script-space-t210` cut from `main` at `9df64ae`; package 0.1.2, Rust 1.99.0
Windows MSVC (T210 is Limit Form's limits, PR #122). Source: the selected Steam build 15194255, validated at every run; the install was
only read.

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/sephiroth-01/`
(`n_hb630.lst.txt`: his script's constants W16 and W32) and the runs `kh2-boss-out/m1`,
`u0`..`u3`, `l0`..`l2` (logs and captures). No Ghidra MCP was used; no new decompiles were
needed.

## What was wrong

The room's scene is the engine's world turned half a turn about X: (x, -y, -z), as the map is
converted, with headings `pi - psi` (T206's spawn fix). Object scripts run in the engine's
frame, but the enemy crate turned their vectors with a y flip only. Script space was therefore
the engine **mirrored front to back**. Positions computed from live objects stayed consistent,
so most fights looked right; what a script takes from the engine itself did not:

- constants: Sephiroth's Meteor spot (engine -750, -600, 2250) landed at scene z +2250, off his
  arena's floor, instead of -2250; his arena centre W32 and the warp points drawn around it were
  mirrored;
- fixed turns: a party limit's sweep (ATKP 665) went the mirrored way round (it now sweeps from
  +x to -x);
- headings a script reads or writes (`obj_set_rot`, `obj_dir`'s w);
- model-local offsets (`obj_apply_bone_matrix`), which mirrored in x.

## What changed

- `native()` turns world vectors as the map is turned: (x, -y, -z). `native_local()` turns a
  model-local offset: (-x, -y, z) (the engine's model frame is the raw model turned half a turn
  about Z under headings `pi - psi`). `native_yaw()` maps headings `psi <-> pi - psi`. A unit test
  checks the identity they rest on: an engine offset placed by an engine heading lands where the
  scene placement puts it, for any heading.
- Every trap that moves a position, direction or heading across the script boundary uses them:
  the 39 `native` sites, about 25 hand-written y flips (`obj_get_moved`, the target, line,
  ground and effect vectors, route positions, the light cycle, Chicken Little's aim, Trinity
  points), the raw reads used as world directions (`obj_wish_dir`, `obj_set_dir`,
  `obj_turn_dir`, jumps, hops, the limit hover, vacuums), and the heading traps (`obj_set_rot`,
  `obj_dir`, route facing). Jump and hop vectors keep their height and speed lanes.
- An object in camera space (Stitch in his limit) holds view coordinates: its own position and
  heading keep the y-only flip [I].
- Host side: the room's script effect vectors (opcodes 38/41), Sora's bone matrix for scripts
  (engine heading and translation), and the flare pool's push shapes.
- A Stitch shot's strike now carries its origin (the view's origin moved up to 200 on x and y,
  FUN_1401b8250), and the room knocks the enemy away from it rather than along the summon's
  facing [I: the direction rule of FUN_1403cf830 for a strike is not traced].
- `Thread::frame_base` wraps as the release build already did (a debug build stopped on the
  overflow in the party-limit tour).

## Steps

1. Classified every vector or angle crossing in the trap layer (world, model-local, heading,
   view space, pure script math).
2. Converted them; updated the four tests that pinned the y-only flip; added the identity and
   constant tests.
3. Compared unattended tours against `main`'s release build, tick by tick.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and tours against the install (arena-07 config for the
summon and party-limit tours, hb01 programs 75 for Sephiroth).

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`),
  workspace build: PASS. Release helper: see the pull request.
- Sephiroth (`m1`, HP 850): Meteor goes to scene (-750, 600, -2250), the engine's spot; his warps
  land on the floor; the fight runs to the end of the tour.
- `--party-limit-tour` (`l2` against `main`'s `l0`): identical tick by tick except the ATKP 665
  sweep, now mirrored to the engine's side (from +x to -x); 225 hits against 226; five limits,
  27 follow-ups, exit 0.
- `--summon-tour` (`u3` against `main`'s `u0`): identical until Stitch's limit at tick 4133; his
  shot hits the Shadow's back (the view's origin is behind Sora, the Shadow faces away), it takes
  the back reaction (motion 13 instead of 11) and the rest of that volley misses it (summon hits
  78 against 93: 883 ×1 and 1210 ×1 instead of ×13 and ×4). Every other summon, appear event and
  limit follow-up matches; exit 0, no faults.

## Uncertainty

- Nothing was compared with the original game. The frame identity is derived from the map's
  conversion and T206's heading rule; it holds in the tests, and the Meteor spot and the limit
  sweep moved the way it predicts.
- Camera-space objects keep the old conversion [I]; the room's view handling for Stitch is its
  own.
- Stitch's shots landing less often follows from the strike's direction rule, which is [I].

## Verdict and next task

PARTIAL. Script vectors, offsets and headings now cross in the engine's frame; nothing is
claimed 1:1 by observation.

Next: (1) ground mode bringing an airborne object down (Sephiroth after Heartless Angel);
(2) the strike direction rule (FUN_1403cf830); (3) a side-by-side capture against the installed
game.
