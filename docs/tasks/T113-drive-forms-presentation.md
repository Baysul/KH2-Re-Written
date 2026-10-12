# T113 — Drive Forms, second pass: starting Drive, Limit Form's limits, drive effects, afterimage, level-up panels and voices

Owner Claude, 2026-10-11. Gates P18/P21 (test-room scope). Continues T108 from `main` at 0dddc10. User request: the forms 1:1 (animations, effects, attacks, growth abilities, gravity and feel), Drive usable from the command deck (the room started with 0 Drive), and the drive issues on GitHub (#13 Limit Form limits, #14 drive particles / afterimage / level-up panel, #15 Master/Final/Anti playtest).

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Ghidra 12.1.4 headless, read-only projects. Private evidence under `research/playable-room-20261008/`: `limitform-01` (the `limitform` BDX threads TR22/TR23, Strike Raid's Keyblade, Infinity's bullets, the colour filter), `drivefx-01` and `drivefx-02` (drive effects, PAX locations, level-up panel, HUD burst/flash, afterimage, member fade, voices), `forms-data-02`, and the unattended runs `drive-03/autoplay-0[1-6]` with their logs.

## Findings (executable and installed data)

1. **0 Drive**: the room's gauge started empty and nothing filled it before a fight, so Drive stayed grey. Interactive rooms now start with the gauge full [test-room choice, as T108's F11].
2. **Limit Form** (BDX `limitform`, FUN_1403f29e0, FUN_1403f36c0): the Magic slot is "Limit" (0x2c3) opening submenu 12 with Sonic Rave, Last Arcanum, Strike Raid and Infinity; shortcuts are limits in the form. A limit costs MP (greyed when short, all MP while charging); the TR22 thread runs while the motion is W0 or 241..248, offers follow-ups through frame ranges 16/17/20 as the reaction command (Rave/Finish, Bash/The End, Raid/Judgment, Shoot/Impact/Giga Impact), TR23 queues the follow-up motions. Strike Raid throws a Keyblade (speed 30, homing 5°/tick to 120 units, held 120/150 above ground, ATKP 1866 r160 / 1867 r190); Infinity fires bullets (ATKP 1862 r80, 16 a tick, three lock-ons); the colour filter ramps in over 16 and out over 30 frames; finishers give Limit Form EXP.
3. **Drive effects** (FUN_1403f7b70, P_EX100 PAX): change effects at the start, form effects bound to bones, revert effects 15..27; the old object's effects fade. The gauge's rebind (FUN_140180900 / FUN_140180b00) spawns the ring burst (group 0x82) on a Drive and a revert and the return flash (0x35) on a revert; the FORM frame starts at its loop group 51.
4. **Afterimage** (FUN_14015f030, priority 75000): from T0+30 the previous frame is drawn over the scene at 0x70/128 (out = 0.875·prev + 0.125·current); at the swap it fades by 0.05 a step and stops with the task about four ticks later; none on a revert.
5. **Level-up panels** (FUN_14017ddc0, FUN_14017e1d0, FUN_14017d6f0, FUN_1403e8380): up to three queued, started as one batch stacked by the group's padding (−44 / −64); 'gaug' groups 0x58..0x63 by form; title = form item name, "%2d" level, "Form Gauge Powered Up!" (0x43f2) and the new form ability; a second "Sora" (0x51f) panel lists his growth and bonus abilities.
6. **Voices** (frame trigger 14 → FUN_1403d69b0 → FUN_1403cdb60): 00battle `vtbl` rows {char, n, priority, 5 × (sound, weight)}; a weighted roll on the battle LCG (×0x10dcd), else a uniform pick among unweighted sounds; Sora's bank is `voice/us/battle/<world>0_sora.win32.scd`; none in Antiform.
7. **Member fade**: 16 steps (the member is removed on the 16th).

## Implementation

- `player_sim::limitform`: the limit thread, follow-up offers, Strike Raid's Keyblade, Infinity's bullets, MP colours, the filter ramp and Limit EXP. `player_sim` clips gain script windows (16/17/20/26), events (7), marks (9) and voices (14); `Action::Scripted` plays any motion id with root motion and a queue.
- `test_room/drive_fx.rs`: PAX effects for the change, form and revert, bound to the kit's bones. `test_room/drive_room.rs`: the Limit row and submenu, limit ticking and hit volumes, the thrown Keyblade, the scene tint, the afterimage ramp, level-up panel text and `Voices`. `windows_shell::gauge`: burst and return flash. `windows_shell::levelup`: the panel queue, sprites and text. `cutscene/blur.rs`'s feedback cameras are reused for the afterimage.
- The unattended run adds Limit Form with Infinity and Strike Raid (ticks 7480..8390).

## Checks

PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked` (effect_vm 26, 12 new synthetic: loops, catch-up, YXZ, wood billboard, SRandDown, BornRnd3, PointAp, LoopZ scroll, tail ring, ribbon drift/fade/swap, KeTh bolt); PASS `cargo build --workspace --locked`; PASS smoke `cargo run -p windows_shell --locked -- --frames 180`; PASS release autoplay `drive-03/autoplay-06` (exit 0): Drive refilled, Limit Form entered at 7490 with the afterimage over the change, Infinity (683) with its Shoot follow-up, Strike Raid (704) with three Raids and Judgment hitting with ATKP 1866/1867, Sora's battle voices from bank 5113. A private harness ran every drive element 1200 frames with no unsupported program and no NaN. NOT RUN: interactive playtest of Master, Final and Antiform (#15) by the owner; comparison captures against the installed game.

## Uncertainty

- The afterimage's history is the scene without last frame's HUD (the exe's prevFrameTex holds the whole frame), and its first frame shows the plain scene [I]; the exact tail length is [U] (four ticks taken).
- Level-up panels: the title uses the system font (the exe's font 1 is not loaded here), the ability lines omit the inline icon glyph, and the text baseline is inferred.
- Voices draw from their own LCG copy rather than the shared battle RNG, so picks differ from the exe's sequence of draws.
- The +3 yield ticks after motion 0x34 before the swap are not modelled.
- `effect_vm` now runs every program the drive elements use (P_EX100 1..27, the forms' 1..12 and 100..102, 00effect 16): loop ranges with catch-up and restart, kinds 6/7/11, 48, 77, 90, 93, 115, 116, 126, 128, 129 (KeTh lightning), 138, 157 (KeShpTail3X), 172, 181, 213..217, 219, 238 and ribbon modes 0x242/0x244. 238's frame-buffer refraction draws nothing; lightning width uses the exe's startup camera scale; ribbon and lightning alphas are averaged per primitive; subtractive lightning is drawn as alpha [I]. Non-drive elements in the same files still report kinds 47, 51, 65, 85, 89, 96, 99, 120, 121, 123, 155, 160, 178, 188, 189, 191, 200, 211, 212, 218 and 234.

## Verdict / next

PROGRESS: the room starts with a full Drive gauge, every form enters from the command deck with its change effects, afterimage, gauge burst and level-up panels, Limit Form's four limits and follow-ups run from its Limit menu, and Sora speaks in battle. Next: the owner's interactive check of Master, Final and Antiform; draw 238's refraction; the remaining non-drive effect kinds with the attack effects.

## Addendum: remaining attack and limit effect kinds

PASS `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (effect_vm 35, 11 new synthetic), `cargo build --release --features skate3 --locked -p windows_shell`; PASS release autoplay `drive-03/autoplay-09` (exit 0, no `drive_effect_unsupported` events; Limit Form, Infinity and Strike Raid start; form change at 7650).

Translated from decompilations (`combatfx-01`, `effect-03`): 47, 51, 65, 85, 89, 96, 120, 121, 123, 211, 212, 234 (FUN_1404c15f0 to FUN_1404c6d90).

Still unsupported (NOT RUN in a game, decisions pending): 99 (integer versus float angles in the exe), 155 and 160 (sprite emission paths not mapped), 178 (child allocator), 188/189/191 (line-chain draws), 200 (ribbon-bone draw), 218 (draw mode bits differ from 219). Known inexact: 121/123 ignore the camera-offset flag, 85 runs only on even part frames.

