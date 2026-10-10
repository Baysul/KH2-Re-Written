# T065 — Dedicated test arena

Owner Claude, 2026-10-09. Gates P18 (test-room scope). Starts from T064 (`b9d6d1c`). User request (2026-10-09): build an actual test room instead of using Twilight Town assets.

## Implementation

- `room::arena(half_size, wall_height, tile)` generates the room in scene coordinates. It is our own geometry and texture, not game data:
  - a square, tiled floor at y = 0 with floor flags;
  - four walls with wall flags, which also serve as camera collision;
  - a procedural slate-grid texture.
- `test_room` configuration:
  - `room_selection` is now optional, and `arena` selects the generated room.
  - The HUD and command menu use the world-independent sheets `field2d/us/zz0field.2dd` (kh2_fifth 987) and `zz0command.2dd` (kh2_fifth 986) instead of the Twilight Town ones.
- The private bundle's launch file now points at `test-room-arena-01.json`, which has a 3000×3000 arena, 400-unit walls, 150-unit tiles, a spawn at (0, 10, −600) and the Shadow 250 units ahead.

## Evidence (private)

`research/playable-room-20261008/test-room-arena-01.json` and `autoplay-30`.

The capture shows Sora and the Shadow on the grid floor inside the walls, with the generic blue command menu and the gauge. The combo hits at ticks 419/446/486 for 3/2/6 damage, as in T064.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS 180-frame smoke; PASS autoplay-30; PASS release `test_room` build. NOT RUN: interactive playtest.

## Uncertainty / next

- The arena is a test-room construct and has no original counterpart.
- The zz0 sheets' selection rule (FUN_1402e5380) is not traced. The game uses them when no world-specific file applies.

Next: the Shadow's AI, enemy attacks and damage to Sora.
