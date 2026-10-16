# T150 — VFX and SFX pass: hit sparks, item and prize effects, map effects, effect VM, swing trail

Owner Claude, 2026-10-16. Gate: P22 (field and combat presentation), PROGRESS only. Source: Steam build 15194255 through the session-validated source (`source-manifest-02`). Private evidence: `research/playable-room-20261008/vfxsfx-01/` (decompiles of FUN_14040dd70, FUN_14040d810, the ppp program table at 0x140779a00, FUN_1404d1960/d11e0/d1d00/d1480, FUN_1402c5f70; `effect-census.tsv`).

## Done
- Enemy attack landing on Sora starts the attacker's own PAX hit element (category mask rule of FUN_1403d1b40) with its sound (Shadow 201016/201017), for the room Shadow and the battle-gallery enemies (M_EX010/020/990).
- `drive_fx::Placed` requests (world point, room enemy, map element, weapon) beside Sora's own; the enemy dead effect uses it.
- Item use: system element 0 (se 100) at the trigger, the item's element 1..4 (se 101..104) when the heal lands (FUN_1403f6b30).
- Prize orbs: bounce sound (first two bounces) and pickup sound (table 0x1405c7390).
- Map PAX elements start per background group and end with the group or area.
- Effect VM: RandUp/Down variants, SRandUpFV, HCV draws, Matrix XZY/ZYX and loops, SnoScaleAll, MoveLoop, DrawMdl, DrawMdlTs, DrawMatrixNoRot, VertexApAt, VertexAttend, KeThRes8/24/40. Frame-buffer refraction programs (KeMdlDtt, KeShpDtt) are not drawn. Census: hidden installed effects 2173 -> 775.
- Keyblade swing trail (pppRyjDrawShipolyBone): ring, natural cubic spline, quad strip with fade, per-corner quad colours, BonePos feed from the blade bone.
- `cargo run -p main --example effect_census` lists the programs the VM lacks.

## Update (second pass)
- Effect VM: pppKeShpTailX (153) and pppKeShpTail2X (155) sprite tails (FUN_1404cc750/cc960, FUN_1404cd550/cd760). Census: hidden installed effects 775 -> 643 (433 of those belong to the magic work's four programs).
- Ground dust: frame trigger 19 starts the map PAX category element for the floor's attribute (table 0x1405d4ff0) and the foot id, at Sora's position turned along his heading. Not seen in a run (Twilight Town's early floors have no dust element); foot offset and ray test are not modelled.

## Checks
PASS fmt, clippy (warnings denied), `cargo test --workspace --locked` for the first batch; effect crate tests and clippy PASS for the later batches. PASS unattended arena run (hit sparks 201016/17, item 100..103, prize 11/12/15/16, trail element 10 drawn). NOT RUN: listening or capture comparison with the installed game.

## Uncertain
- Trail spawn rule (range 12 start spawns element 10 once) and bone length (21.05 from mesh extent) are inferred; the consumer of motion flag 0x800 was not found.
- Map element sounds are not positional; map PAX has no occluder (WA_t) handling.
- Item element 0 started at Sora's origin, not attach point 8; Mega items affect Sora only.

## Still missing (next)
Ribbon tails (155 Tail2X, 153, 154), mega-model colour/snapshot buffers (0x101/0x106), ribbon pair modes (0x241/0x243), lens flare family (188/189/191/221/223/224/225), RyjMatrixWorldXYZ (99), MegaBirth user-control variants; ground dust (frame trigger 19 with map PAX categories), enemy motion sounds/effects for non-Shadow enemies, KO/revive/guard/level-up effects, field objects (chests, save point), pickup sparkles and orb models. Magic thread owns spell VFX/SFX and four VM programs (DrawMdlLoop, MatrixLoopYXZ, VertexApLcLoop, KeZCrct mode 3).
