# T096 — EXP, level-ups, prizes and the Drive gauge

Owner Claude, 2026-10-10. Gates P18/P21 (test-room scope). Starts from T095. User request: rewrite the ability and combo system 1:1; this is stage 3b and the first part of stage 6.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `damage-02` §11, `items-02` Part A2 and P.6, `drive-01` §1, and decompiles of FUN_1403ecf30, FUN_1403ed7d0 and FUN_1403e60c0 under `combo-01/g/lvl*`. Autoplay evidence is in `combo-01/autoplay-s8`.

## Findings (executable and installed data)

1. **Kill EXP** (FUN_140410af0): (enmp EXP × lvpm row EXP + 99) / 100; three quarters (at least 1) on Critical; times the Experience Boost count at or below half HP. The Shadow gives 15 × row EXP: 1 at level 1.
2. **Level-up** (FUN_1403ecf30): while total EXP reaches the current level's `lvup` threshold the level rises, to 99. The row sets Strength, Magic, Defence and AP and names the ability items learned (u16 at +8, +10, +12, one per growth choice); HP, MP and Drive growth are not in these rows (they come from FUN_1403edf90's bonus table, not wired here).
3. **Prizes** (FUN_1403dc170): the przt counts scaled by Jackpot, munny turned into drive by Drive Converter, item rolls with Lucky Lucky; the Shadow's death scatters przt 2 (1 small HP, 1 small munny, item 280 at 4%). A prize is collected within 120 (+ Draw) horizontally and 30 vertically of its centre; HP orbs add HP flat, MP orbs go through the MP Charge rules, drive orbs through the central gain, munny to the wallet.
4. **Drive gauge** (FUN_1403c0ad0, FUN_1403a8800, FUN_1403a9cf0): 100 points per bar; a hit adds its ATKP DriveDrain (×0.75 on ZAKO), ×2 × Drive Boost while MP charges; damage taken adds damage/MaxHP × Damage Drive.
5. **Forms** are unavailable to a lone Sora by the exe's own rules: the party requirement (FUN_1404005e0) is missing, so the Drive row stays greyed.

## Implementation

- `player_sim::drive` (gauge add, pay, hit and damage gains), `player_sim::prize` (drops, orbs, pickup, kill EXP, level lookup); `battle_tables`: enmp EXP and prize id, `prize`, `PlayerLevel.abilities`.
- `test_room/prize_room.rs`: kills grant EXP and level-ups (stats follow the rows; the camp's Status shows the level, next EXP and stats and receives learned abilities), prizes scatter as coloured placeholder spheres, fall, and are collected; hits, damage and F9 move the HUD's Drive gauge.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS release autoplay `autoplay-s8` (exit 0, started 2 EXP short of level 2): EXP 1 per kill, level 2 at 40, HP and munny prizes collected, drive partial 8 → 12 → 15 from hits. NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

- The prize fall and bounce are inferred (FUN_1404132d0 not traced): drawn velocities are scaled by 0.1 per tick, gravity is the actors', a bounce keeps half the speed. Prize models, sparkle effects and the item treasure actors are not drawn; item drops are logged only.
- Level-up learns the first growth choice's ability, unequipped; the message pops, HP/MP/Drive growth and the level-up fanfare are not reproduced.
- Form commands, the Anti/Final roll and form EXP are specified (drive-01 §2–§5) but not implemented: they need the form models, motion sets and PTYA sets, and a party.

## Verdict / next

PROGRESS: kills give EXP and prizes, level-ups change stats, and the Drive gauge fills by the exe's rules. Next: Drive forms (models, motions, abilities), which need configs for the form actors.
