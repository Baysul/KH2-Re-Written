# T220 — The map hit record of `bghit_check_line` and its getters

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do", then "keep going"), from T219's finding. Gate: enemy AI (P19/P20
work; nothing here is inferred as a gate pass). Prerequisites: T219 (the collision-class line
query). Build identity: branch `feature/bghit-record` cut from T219's branch at `acff63a`, merged
with `main` at `1c8d0e8`; package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam
build 15194255, validated at every run; the install was only read.

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/boss-01/` (plain
`analyzeHeadless` outputs `out/j7`, `j8`, `m3`, `m4`), the trap census
`research/enemy-ai-20261016/trap-map.json`, and the runs `kh2-boss-out/cmp5/`. No Ghidra MCP was
used.

## What the executable does

- `bghit_check_line` (6:51, FUN_14053eb70, four arguments) takes the script's hit record, the
  two points and a mask, and calls FUN_14040c2b0(record, a, b, mask): the record's +0x14 becomes
  -1 and +0x18 0, the segment ray (FUN_140171350, or the attack ray FUN_140171090 when mask bit
  16 is set) writes the crossing to the record's +0 and the polygon to +0x14, and the trap
  answers whether +0x14 is not -1.
- The getters take the record: 6:52 (FUN_14053ec80 -> FUN_14040c3c0) the normal of its polygon
  (FUN_140170ac0), 6:53 (FUN_14053ecd0) whether +0x14 is not -1, 6:54 (FUN_14053ed00) its
  crossing, 6:55 (FUN_14053ed40 -> FUN_14040c380) its polygon's surface nibble (FUN_140170910).
  6:52 and 6:54 hand back the same static vector (DAT_1407883d8).
- The census counts 32 models whose scripts call 6:51, 26 for 6:54, 19 for 6:53, 17 for 6:55 and
  9 for 6:52.

The crate read the two points from the first two arguments (the record and `a`), kept the
crossing in shared state, gave 6:52 "the last hit", and had 6:53 and 6:55 answer 0 always.

## Steps

1. Decompiled the five handlers and the record readers (`out/j7`, `j8`, `m3`, `m4`).
2. 6:51 writes the script's record (crossing, polygon, collider) and answers from it; 6:52..6:55
   read it; the shared crossing state is gone.
3. `Env::line_hit_polygon`, `Env::map_surface`; the room answers the polygon, its normal and its
   surface on a game map (`collision::World::polygon_surface`).
4. A unit test runs 6:51 and the getters against a synthetic wall.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and tours against the install.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`; one new
  test), workspace build: PASS. Release helper: PASS.
- Tours against `main`'s release at `1c8d0e8` (`cmp5/`, the generated arena), all exit 0:
  - summon tour: identical tick by tick;
  - Limit Form tour: identical but for a 2-tick shift of the opening form change (it varies run
    to run);
  - party-limit tour: identical until Trinity Limit (`trinity.bar`, from tick 4720). Its script
    calls 6:51; with the points read from the right arguments the line to the enemy is clear on
    the open arena, and from tick 4880 the limit stands at z -290 (200 short of the enemy) where
    `main` keeps it at 0. Party hits 234 against 230 (attack 155: 15 against 11).
- Sephiroth (`cmp5/seph`, HP 2000 from tick 300, 4000 ticks): exit 0; his script calls none of
  these traps.

## Uncertainty

- Nothing was compared with the original game. Trinity Limit is the only toured script that calls
  these traps, and it runs on the generated arena, where only vertical lines can hit; the record
  on a game map is checked by the unit test only.
- [I] Where the host cannot name the polygon (the generated arena, and the attack ray, whose
  result carries no polygon index here), the record gets polygon 0 so that it still reads as a
  hit; its normal is then straight up and its surface 0.
- The dynamic colliders (record +0x18, FUN_140173500 on a collider's world) are not modelled:
  the map is the only world.

## Verdict and next task

PARTIAL. The five traps follow the executable's record; nothing is claimed 1:1 by observation.

Next: bodies for the ordinary enemy classes, the attack ray's polygon, and a side-by-side capture
against the installed game.
