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

NOT RUN yet (filled in below when run).

## Uncertainty

- The afterimage's history is the scene without last frame's HUD (the exe's prevFrameTex holds the whole frame), and its first frame shows the plain scene [I]; the exact tail length is [U] (four ticks taken).
- Level-up panels: the title uses the system font (the exe's font 1 is not loaded here), the ability lines omit the inline icon glyph, and the text baseline is inferred.
- Voices draw from their own LCG copy rather than the shared battle RNG, so picks differ from the exe's sequence of draws.
- The +3 yield ticks after motion 0x34 before the swap are not modelled.
- Effect programs the effect VM cannot run yet are logged (`drive_effect_unsupported`) and skipped.

## Verdict / next

Filled in after the checks.
