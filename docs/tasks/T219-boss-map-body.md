# T219 — The boss moves through the map solver; the random-position search uses its class

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do"), from T217's finding: Heartless Angel could send Sephiroth under raised
map geometry, where the crate's default enemy movement put him on top of it and his Split Flash
stalled. Gate: enemy AI and physics (P19/P20 work; nothing here is inferred as a gate pass).
Prerequisites: T217, the `collision` crate (Sora's map solver), T206/T209/T213. Build identity:
branch `feature/enemy-map-body` cut from `main` at `9019ddc`, merged with `main` at `c3d0932`;
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build 15194255, validated at
every run; the install was only read.

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/boss-01/` (plain
`analyzeHeadless` outputs `out/i1`..`i9`, `out/j1`..`j8`, `out/k1`..`k5`), the earlier
`research/playable-room-20261008/tt-world-01/collide-01/collide-spec.md` (§1.2, §2, §3), and the
runs `kh2-boss-out/g3`..`g7` and `cmp4/`. No Ghidra MCP was used.

## What the executable does

- **The body.** FUN_1403b3200 reads the first BAR-type-23 collision entry of kind 0 or 4
  (FUN_1403c9bc0(.., 0x11)): radius +0x10, height +0x12, the sphere's centre +0x0e above the feet,
  mode 1 for kind 4 (collide-01 §2). The boss constructor FUN_1403f8da0 calls FUN_140411700,
  which sets the collision class to 1, or 2 when the object entry's +0x48 bit 3 is set (`out/i9`;
  the default enemy constructor FUN_140419640 sets 1, `out/i7`). Sephiroth's entry gives radius
  150, height 150, centre 170, kind 0, class 1.
- **The move.** FUN_1403b89a0 hands every object with a body to FUN_1403b9090, which solves its
  displacement with FUN_14016fec0 (collide-01 §3), projected into the floor plane when grounded,
  airborne when FUN_1403ba9a0, the mode's flag bit 1 or a hold range says so. The solve's
  CB+0x74 bit 0 (`+0x744`) is the "no floor" T217's leave branch reads, and the land branch
  lands when not rising, FUN_1403ba1e0 allows it (no range 3) and that bit is clear. The class
  picks the polygons: class 1 collides with attribute bit 5, class 0 (Sora) with bit 4.
- **The random position.** `bg_get_random_pos` (FUN_14042ead0) passes mask 1 to the ring search
  (FUN_1403f3fa0), `bg_get_random_pos_air` (FUN_14042eb90) passes 2. Every map test in the
  search is FUN_14040c2b0 -> FUN_140171350: the segment ray FUN_140173b30 with the polygon
  filter cleared and the mask as the class (`out/j1`..`j6`). So the ground search only accepts
  floor that enemies of class 1 stand on. The other map-line traps pass their own masks
  (`out/j7`): `can_see` (1:347) and `bg_ground_pos` (1:141) pass 4, `bg_cross_pos` (1:135) and
  `bghit_check_line` (6:51) pass the script's fourth argument; a mask with bit 16 takes the
  attack ray (FUN_140171090) instead.

## Steps

1. Traced the class leave wrappers to their vtables (`out/i1`, `i2`), the boss constructor's
   class (`out/i4`..`i9`) and the search's mask (`out/j1`..`j6`).
2. `enemy::env::MapBody` / `MapMove` and `Env::collide`; `Sysobj` keeps the body, `+0x744` bit 0
   and the floor normal, solves every move through the host when it has a body, reads `+0x744`
   for leaving the ground, and lands from the solve (`Sysobj::touchdown`).
3. Host: `MapWorld::body_move` (FUN_1403b9090's projection, FUN_14016fec0, FUN_140170c10, no
   shared state with Sora's), the boss body from the model's collision entries and the object
   entry's flag word, and the script objects' wall circle only for objects without a body.
4. `Env::line_hit_class` and `collision::World::segment` (FUN_140171350); the random-position
   search, `can_see`, `bg_ground_pos` and `bg_cross_pos` pass their masks; the room answers any
   segment on a game map (bit 16: the attack ray) and keeps the vertical floor test on the
   generated arena.
5. Unit tests for the body path (leave, land, range 3 holding the landing) and the segment's
   class and filter; tours against the install.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and tours against the install.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`; new tests
  in the enemy and collision crates), workspace build: PASS, also after merging `main` at
  `c3d0932`. Release helper: PASS.
- Sephiroth (`g6`, HP 2000 from tick 300, 4000 ticks): Heartless Angel ran three times; each
  time the search put him on enemy floor, he rose to y 590, fell when it ended, landed (motion
  5) and fought on. After the second one the reaction command succeeded (the Block counter).
  Before the search's class was fixed (`g3`..`g5`) it had picked a spot whose floor is Sora's
  class only: the solver found no floor there and he fell from the start of the motion; with
  the class that spot is refused.
- Against `main`'s release at `9019ddc` (`cmp4/`, the generated arena, which keeps the old
  movement): Limit Form, party-limit and summon tours identical tick by tick but for the exit
  tick (which already varies run to run). All exit 0.
- Sephiroth on the boss map (`cmp4/meteor`, HP 850 from tick 300, 6000 ticks): Meteor ran (29
  ticks of `atk_meteor` samples), highest point 600 (Meteor's spot); exit 0. `cmp4/death` (HP
  30): he died, the mission cleared and the clear overlays ran (48 overlay-camera samples);
  exit 0.
- After the merge with `main` and the three traps' masks (`g7`, same settings as `g6`): the same
  Heartless Angel / fall / land / Block sequence. In that run a revenge dash took him up raised
  ground at the arena's edge (y 196 near z 813) and he dropped to a lower surface (y -53), where
  he stood until his next act. See the area limit below.

## Uncertainty

- Nothing was compared with the original game.
- Only boss-class objects (objentry type 3) get a body. The other enemy constructors are traced
  for their class (1, or 1/2 by the same flag) but not for which objects use them, so ordinary
  enemies keep the host's movement and their wall circle.
- Not carried out: the grounded edge refusal (FUN_1403bab20 / FUN_1403ba750) and the enemies'
  area limit (FUN_14040c4b0, obj+0x6cc bit 2), the head bump (obj+0x120 bit 22), body-versus-body
  pushes (FUN_14040ad70), riding platforms (CB+0x84) and FUN_1403c8a40's range evaluation at
  motion start.
- Found, not translated: the enemies' area limit. FUN_1403b9090's grounded branch for objects
  with obj+0x6cc bit 2 tests the step against the shape that FUN_14040c4b0 returns (DAT_142ae9f18).
  That shape is registered by the area's zone-event handler FUN_1403fe320 for zone type 7
  (FUN_14040c550, `out/k1`..`k5`); a step that would leave it is cut. Without it the boss can
  walk onto the arena's surroundings. The old movement had the same freedom (it climbed any
  floor up to 200 units), so this is not new here.
- Found, not fixed: `bghit_check_line` (6:51, four arguments) takes a hit record, the two points
  and the mask (FUN_14053eb70), and 6:52..6:54 read that record. The crate reads the points
  from the first two arguments and keeps the result in shared state; the record's layout
  (FUN_14053ed00 and its neighbours) is not traced yet.

## Verdict and next task

PARTIAL. The boss's map moves, its leave and land tests and the random-position search follow
the executable; nothing is claimed 1:1 by observation.

Next: the enemies' area limit (zone type 7), the 6:51..6:54 hit record, bodies for the other enemy
classes, and a side-by-side capture against the installed game.
