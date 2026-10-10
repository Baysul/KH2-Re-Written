# T058 — matching Sora's motion evaluation to the original routine

Owner Claude, 2026-10-08/09. Gates P15/P18 (test-room scope). Starts from T057 (`5b2c113`). User request: use the local tools to understand the original KH2FM animation/IK and make it 1:1; the user approved downloading Ghidra.

## Sources and tools (all private, outside Git)

- Reference evaluator: OpenKH `OpenKh.Kh2AnimEmu` (pin `7a3b945c538d32c6a285128c98aefba093f52ceb`, private checkout under `%USERPROFILE%/KH2FinalRewrite-private/research/tools/openkh`), a static recompilation of the PS2 KH2FM motion routine with its bundled EE RAM image. Built with the Windows .NET Framework C# 5 compiler into `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/anim-oracle-01/oracle.exe` through a small private adapter (RAM image loaded from file; no source change to the upstream routine). Run only on Sora's model/motions exported privately from the selected Steam install (`motion_ik_probe … <payload-dir>`).
- Ghidra 12.1.4 official release (`ghidra_12.1.4_PUBLIC_20260921.zip`, SHA-256 `ddac49f9…d4db`, matches the pinned value) under `%LOCALAPPDATA%/KH2FinalRewrite/tools/`. Headless auto-analysis of the KH2FM PC executable (its `.text` is not encrypted; entropy 6.46, no DRM section) in `research/ghidra-kh2-01`. The `calc_motion` job dispatcher (`0x1403beec0`, job `0x1403bf4e0`) was located and its callees exported; the PC evaluator functions were not yet fully recovered (agent runs stopped on usage limits).
- Private recovery notes from the recompiled routine are in the session scratchpad and workflow journals; no decompiled code enters the repository. The Rust implementation is this project's own structure.

## Findings applied

1. Tick time is in key-time units; clips play from FrameStart (header word 32) to FrameEnd and loop to FrameReturn (word 35). T056/T057 had used word 32 as a loop start.
2. Curve sampling: binary search; Hermite uses the left key's out-tangent and the right key's in-tangent as slopes per key-time unit; pre/post cycles: hold, linear extrapolation, repeat, zero.
3. Local transform = S, Rz·Ry·Rx, T; world = parent × local; scale is not inherited (accumulated world scale only scales a joint's own output rows).
4. Constraints gate on the temporary-active byte only; the activation table is not read by the routine.
5. Direction (type 3): in the parent frame, X aims from the joint's (constrained) local translation to the source, up reference is the parent's +Y (`y ∝ (d×Y)×d`, `z = d×y`), then the joint's own animated X rotation is applied as roll.
6. Position limit (12): applied in the parent frame around the source position. Rotation limit (13): applied to the joint's local rotation channels. Limiter (48 bytes: bits, damping width +8, strength +12, min +16, max +32): type 0 or global uses a zero centre; type 1 is a sphere on min.x (hard limit leaves the value unchanged; soft limit `len = (R−w) + w(1−exp((s−1)·excess/w))`); other types clamp enabled axes with the same exponential softening.
7. Two Position constraints on one joint use the midpoint. Expressions write rotations in degrees (×0.0174533) and read rotation channels in degrees.
8. IK chains: end joint with translation+rotation flags keeps its own local rotation in character space; the two-joint hinge is the chain parent's Y axis projected perpendicular to the hip→target direction.

## Evidence

`motion_eval_compare` compares every model-bone world matrix against the reference for every frame of the 12 selected clips (`anim-oracle-01/ref-02/`). Maximum translation error over all bones and frames:

| Clip | Before T058 | After |
|---|---|---|
| idle 0 | 20.41 | 0.032 |
| walk 4 | 3.74 | 0.021 |
| run 8 | 44.01 | 0.81 |
| jump 14 | 1.90 | 0.030 |
| fall 18 | 3.50 | 0.12 |
| land 22 | 3.32 | 0.023 |
| A300 604 | 33.49 | 3.02 |
| A301 608 | 31.01 | 1.66 |
| A304 620 | 9.94 | 0.15 |
| A330 724 | 42.27 | 0.15 |
| A331 728 | 17.73 | 0.17 |
| A332 732 | 13.46 | 0.036 |

Remaining differences in A300/A301/run start at the two-joint IK solve (`0x124a88`, which also uses the top/middle joints' animated X rotation) and an orientation-constrained toe; that solver is not yet reproduced exactly. Autoplay `autoplay-14` (FrameStart timing) and `autoplay-15` (final, release exe) pass with 15 non-blank captures each.

## Checks

PASS fmt, clippy (warnings denied), workspace tests (146), workspace build, 180-frame smoke, autoplay. NOT RUN: interactive playtest; comparison against the PC executable's own evaluator output (the reference is the PS2 routine, assumed equivalent).

## Next

Recover `0x124a88` (two-joint IK with twist) and the constrained-chain branch; confirm the PC functions under `calc_motion` follow the same math; then attack hit detection.
