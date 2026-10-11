# T091 — Hit records, level rows and the ability terms of damage

Owner Claude, 2026-10-10. Gates P18/P21 (test-room scope). Starts from T090 (`d976699`). User request: rewrite the ability and combo system 1:1; this is stage 2, damage and being hit.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `research/playable-room-20261008/damage-02/damage-spec.md` (Ghidra 12.1.4 on the read-only executable, decompiles under `damage-02/g/`). Autoplay evidence is in `combo-01/autoplay-s2`.

## Findings (executable and installed data)

1. **Per-object hit record** (obj+0xc98, FUN_1403cf600 / FUN_1403cf4b0): a 256-bit hit-once mask by attack id, a combo-group counter, a 30-tick group lock on player-type objects (frozen while the owner is in motions 10–39), and four Interval re-hit slots. A counted hit needs the id absent from the mask and the same group, group 0xff or an expired lock; the id is in the mask afterwards even when the hit did not count. This is Sora's only real post-hit protection: taking damage never sets obj+0xd70.
2. **ATKP level rows** (FUN_1403eb230): rows after the first with the same id apply from their Step; Switch picks odd or even hits past it.
3. **Damage terms** (FUN_1403c12f0): Berserk Charge adds Strength while MP charges; Defender removes Defence below a quarter of HP; Combo Boost / Air Combo Boost scale a finisher (Kind 1 / 2) by min(combo step, limit) × percent; Reaction Boost scales Kind 4; Fire, Blizzard and Thunder Boost scale elements 1–3; Damage Control (the Kingdom Key's weapon ability) reduces a hit by 50% at or below its HP fraction; difficulty scales damage taken by Sora (50 / 100 / 200 / 200) and dealt (150 / 100 / 100 / 125). Cure types 5 and 6 use the max-HP or stat-only formulas.
4. **Kill veto** (FUN_1403d3ba0, FUN_1403d6670): a lethal hit leaves floor + 1 HP when Second Chance (ability 127, HP above 1) or Once More (ability 128, during motions 10–39) applies.

## Implementation

- `package_reader::battle_tables`: `Attack` gains step, switch, interval, combo group, Kind, DriveDrain, Revenge and HpDrain; `attack_steps`, `step_row` and `step_index` read the level rows.
- `player_sim::combat`: `HitRecord`, `AttackIds`, `AttackerMods`, `TargetState`, `Difficulty`, `damage_with`, `apply_damage` and `sora_veto`, with tests of each traced rule. `Shadow` owns a `HitRecord` instead of its instance list and `Shadow::hit` takes a difficulty.
- `test_room`: attacks get ids and combo groups per instance; Sora's attacks use the Shadow's record and the level row of its count, and Sora's ability record feeds `AttackerMods`; the Shadow's claws go through Sora's record, so a second claw group is ignored until 30 ticks after he leaves a damage motion; Sora's damage uses Defender, Damage Control, difficulty and the kill veto. The room's unequipped ability list gains Defender, Second Chance, Once More, the element boosts and MP Rage. The enemy config accepts `difficulty` (0–3, default 1).

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS release smoke and `autoplay-s2` (exit 0, 81 captures; the same hits 3 / 2 / 1 / 6 as `autoplay-s1`, and the same event counts). NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

- Only the first-listed ATKP rows of the room's attacks have level variants, so the level-row rule is covered by synthetic tests, not by a room observation.
- The special battle mode and the Anti-form damage rate are not modelled; Sora's blow reactions, Aerial Recovery, EXP and prizes are traced (damage-02 §7, §11) but not implemented.
- Fire, Blizzard, Thunder and Cure need the magic stage; Berserk Charge needs MP charge.

## Verdict / next

PROGRESS: hits follow the exe's record, level-row and ability rules. Next: the Magic stage (command menu, MP and MP charge, the spell scripts), then items, then Drive.
