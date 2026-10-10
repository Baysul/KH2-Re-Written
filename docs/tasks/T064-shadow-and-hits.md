# T064 — Shadow Heartless, Keyblade hits, damage and reactions

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T063 (`b29a07b`). User goal: 1:1 parity. Direct translation of the executable is authorized; game assets stay out of Git.

## Prerequisites / source

Same selected source and private manifest as T056–T063 (quarantined; full rehash at every launch). Ghidra 12.1.4 on the read-only executable; outputs are in `ghidra-kh2-a4/cb-01..35`. The written behaviour spec is `research/playable-room-20261008/combat-01/combat-spec.md`. Public layouts: OpenKH 00battle.md and ObjectCollision.cs (pin `7a3b945c538d32c6a285128c98aefba093f52ceb`).

## Findings (installed executable and data)

1. **Attack windows.** These are range trigger 10 in the Keyblade's motion set (`W_EX010.mset`), carrying [ATKP id, collision group]. Ground combo: 126/127/129; air combo: 131/132/133.
2. **Attack volumes.** These are `W_EX010.mdlx` object-collision ATTACK spheres on weapon bone 2 (group 1: four r40; group 2: three r60).
   - They are swept from last frame's centre every radius whenever the move exceeds 1.5 radii (FUN_14040f610).
   - They are tested strictly against the target's HIT spheres (FUN_1403c94c0).
   - An attack hits a target once, and a target takes one hit per frame.
3. **Damage** (FUN_1403c12f0): s = Str − Def, clamped by the target's per-10000-of-max-HP cap and minimum, then dmg = ⌈weak × general × Power × s / 10⁶⌉. ATKP byte +5 selects strength or magic.
   - Sora at level 1: Str 5 (LVUP 2 + PLRP 0 + Kingdom Key 3).
   - Shadow (ENMP 34) at TT battle level 1: HP 10, Def 2.
   - Resulting hits: 3 / 2 / 6.
4. **Reactions** (FUN_1403ce980). Reaction 3 is a flinch: motions 10–13 chosen by front/back and the HIT entry's high/low argument, with a slide of KB1 × 0.1 per frame. Reaction 7 is a blow: velocity KB2 along the hit direction and KB1 upward, using motions 16/18 and then 17/19 to land.
5. **Shadow model and motions.** The Shadow is `M_EX020`. Its motion set indexes slots directly by motion id (header byte +0xc bit 0 clear).

## Implementation

- `package_reader::battle_tables`: ATKP/ENMP/LVPM/LVUP readers. `package_reader::object_collision`: type-23 reader. `motion::read_slots`: motions at explicit slots. Each has synthetic tests.
- `player_sim::combat`: sphere test, sweep, damage, reaction class, and an `Enemy` with appear → idle → flinch/blow → land, leave on death, and respawn. Tests check the traced level-1 numbers.
- `test_room`:
  - An optional `enemy` block. The Shadow is loaded, its clips and HIT-bone matrices are precomputed, and it is drawn under the scene root.
  - Each tick, the attack windows of Sora's current clip are evaluated through the hand and blade matrices and tested against the Shadow's HIT spheres.
  - Hits are logged (`hit` events). State events carry the enemy's state.

## Evidence (private, outside Git)

`research/playable-room-20261008/test-room-tt08-09.json`, `shadow-selection-01.json`, `autoplay-28` and `autoplay-29` (logs and captures).

In autoplay-29 the ground combo hits at ticks 419 / 446 / 486 for 3 / 2 / 6 damage. The HP goes 10 → 7 → 5 → 0. The first two hits flinch the Shadow and the finisher blows it away; the captures show the blow.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (163 tests); PASS build; PASS 180-frame smoke; PASS autoplay-29; PASS release `test_room` build. NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

Test-room choices:
- The Shadow's AI (its bdscript is not decoded). It only turns to face Sora.
- Its placement: Shadows do not spawn in tt08. It is placed 250 units ahead of the spawn, and unattended runs move it in front of Sora at tick 380.
- The battle level (1).
- Death: there is no death motion, so the Shadow plays its leave motion and respawns after 2 s.
- The slide and blow brake (SSTM 0.8) and the gravity for blown enemies.

Inferred: the Kingdom Key's +3 Str is a constant from the traced item row.

Not implemented:
- Hit effects, sounds, Drive gain, revenge, and Sora/enemy body collision.
- Enemy attacks.
- Inclusivity of the range-trigger end frame.

## Verdict / next

PROGRESS: Sora's combo now hits a Shadow with the original's volumes, damage numbers and reactions. Next: the Shadow's AI script, hit effects/sounds, and enemy attacks.
