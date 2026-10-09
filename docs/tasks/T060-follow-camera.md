# T060 — Default follow camera from the PC executable

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T059 (`afc77c6`). User goal: camera position should match the original.

## Prerequisites / source

Same selected Steam source and private manifest as T056–T059 (quarantined; full rehash at every launch). The installed PC executable was read (never written) and analysed with Ghidra 12.1.4 in a private project outside the checkout. OpenKH `OpenKh.Engine/TargetCamera.cs` (pin `7a3b945c538d32c6a285128c98aefba093f52ceb`) gave the lead constants; every value used here was then checked against the executable. No decompiled code is in Git; the Rust is a reimplementation of the observed behaviour.

## Findings (installed executable)

1. A camera mode table in `.data` (four rows of field of view, radius, lock radius, radius min, radius max, height curve). Row 0 is 1.5, 420, 380, 250, 500, 0.008. A mode setter copies a row into the camera object.
2. The per-frame camera update dispatches on a mode number; mode 0 is the default field follow. Its rules, as read from the decompiled routines and their constants:
   - Look-at target is the player position 170 units up (world Y points down).
   - Eye target is the look-at target plus the radius rotated by the camera yaw, 150 units lower, then raised by (radius − radius min)² × height curve. At the default radius that is 251.2 units above the feet, 420 behind.
   - The yaw eases toward its target by 0.1745 rad per frame. While the player moved horizontally this frame, the stick is pressed, no manual turn is held and the stick's toward-camera component is below 0.9, the yaw turns toward the behind-player angle by at most 2 (|x|+1)(|y|+1) × magnitude × ((speed − 8) × 0.000174533 + 0.00436332) per frame, and the radius returns toward the mode radius by 0.5 per frame.
   - Manual turn is 3° per frame at full input; manual zoom is 8 units per frame, clamped to the radius limits.
   - The current eye and look-at points follow their targets with a spring: offset d = current − target, f = d · ((previous target − target) × gain), step = ((f/|d|) × 0.5 + (1 − |d|) × 2.5) × gain along d. Gain is 0.07 + (speed − 8) × 0.0006, halved when the frame-time scale is below 2 (60 Hz), and the look-at spring adds 0.03 after halving.
   - The speed term reads a player value compared with 8; Sora's PRTY run speed is 8, so it contributes zero here.
3. The projection takes the field of view as horizontal across a 640×480 virtual screen (512/0.8 by 416/0.8667). Field of view 1.5 gives a vertical field of view of 1.2197 rad.

## Implementation

- `player_sim::camera` (new): `FollowCamera` with the mode-0 row, yaw follow, radius return, manual turn/zoom/recenter, spring easing and vertical field of view, in the game's Y-down world space; four tests.
- `test_room`: the camera ticks with the 60 Hz simulation (world = scene (x, −y, −z)), stick movement is relative to the camera's view direction, the projection uses the derived vertical field of view, and Q/E (or right-drag / right stick) turn, −/= zoom, C recenters. Unattended runs orbit 180° at 3°/frame to check Sora's front.

## Evidence (private, outside Git)

Ghidra project `%LOCALAPPDATA%/KH2FinalRewrite/research/ghidra-kh2-a1/` outputs `cam-01` (references to the mode table), `cam-02` (camera routines), `cam-03` (helpers: angle wrap/approach, player position/facing, moved check, button tests), `cam-04`/`cam-05` (projection). Autoplay `research/playable-room-20261008/autoplay-22` with log and captures.

Inspected autoplay-22 captures: running away shows Sora small in the lower middle of a wide view with the look-at point above his head; after the scripted orbit, battle idle is seen from the front with the station building behind, matching the original's framing at the default radius.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (151 tests); PASS build; PASS 180-frame smoke; PASS autoplay-22; PASS release `test_room` build. NOT RUN: interactive playtest by the user; side-by-side capture against the installed game.

## Uncertainty

Not implemented: camera collision against the map's camera collision mesh (the routine exists and is called for eye targets), lock-on/battle camera modes and the fixed-mode switch from area scripts. Assumptions: the default camera setting (a global that selects 0.4/0.0018 gains instead of 0.07/0.0006 is taken as off); the speed value is the PRTY run speed; the stick's +Y is toward the camera; the PC build widens the horizontal view for 16:9 and keeps the 4:3 vertical field of view. Button bindings are test-room choices.

## Verdict / next

PROGRESS: the default follow camera now uses the original's distances, heights, follow rate and easing. Next: HUD gauges, then camera collision.
