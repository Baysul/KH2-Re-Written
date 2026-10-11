# T094 — Sora's hit reactions: flinch, air flinch, blow, Aerial Recovery, death

Owner Claude, 2026-10-10. Gates P18/P21 (test-room scope). Starts from T092 (`3a14d49`). User request: rewrite the ability and combo system 1:1; this is stage 3a, being hit.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `research/playable-room-20261008/damage-02/damage-spec.md` §4–§8 and §12 (Ghidra 12.1.4 on the read-only executable) and `guard-01/pex100-triggers.txt`. Autoplay evidence is in `combo-01/autoplay-s6`.

## Findings (executable and installed data)

1. **Reaction by class** (FUN_1403d2fe0): class 1 knock-slide only; 2 flinch without a slide; 3 flinch with a slide on the ground, air flinch with a 50-unit hop at knock × SSTM HitBackToJump (0.048) in the air; 4 blow. FRONT/BACK comes from the facing against the hit direction, HIGH/LOW from the struck sphere's argument.
2. **Blow** (FUN_140409d30 → mode 4): velocity (KB2 along the hit, KB1 up) or SSTM BlowSpeed/BlowUp when both are 0; BLOW_FRONT 16 / BLOW_BACK 18 loop until landing, then the land motion 17 / 19, actionable 40 ticks in.
3. **Aerial Recovery** (FUN_1403d6d50): Jump with ability bit 20 after 16 ticks of blow plays motion 40 with a 100-unit hop at speed 10 along the stick; motion 40 carries range 27 (invulnerable) for ticks 0–16. The attack button in the same window starts Retaliating Slash (PTYA 0x28, ability 101) with the counters set to 1/1.
4. **Death** (FUN_140404d60): HP 0 plays motion 54 with no horizontal motion; in the air the body keeps falling.

## Implementation

- `player_sim`: `Hit`, `Player::react`, `die`, `revive`, `range_invulnerable`; actions reuse `Action::Hurt(i)` over `HURT_MOTIONS` (10–19, 40, 54); `Clip.invulnerable` (range 27); `physics::Sstm` gains BlowToHitBack and HitBackToJump; `abilities::bit::AERIAL_RECOVERY`. Seven unit tests cover each class, the blow flight and landing, KB overrides, Aerial Recovery's gates, Retaliating Slash and death.
- `test_room`: the Shadow's hits go through `react`; HP 0 plays DEAD and the room revives after 120 ticks or when healed; range 27 blocks damage; MP Rage gives MP from damage taken; Aerial Recovery is equipped by default; F4 strikes Sora with a no-damage blow (the Shadow's claws are all class 3).

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS release autoplay `autoplay-s6` (exit 0): a blow at tick 1500, Aerial Recovery at 1520 (motion 40, then Fall, Land), the second blow lands into motion 19; captures show the blow and recovery poses. NOT RUN: interactive playtest; comparison with the installed game; autoplay observation of Retaliating Slash (equipped for the next run).

## Uncertainty

- Class 2 hits in the air use the air flinch here; the exe plays the ground flinch motion in the air.
- Super-guard volume (ATKP 124, group 6) of Aerial Recovery is not drawn; only its invulnerability is.
- The blow's wall response, motion-14 trigger 16 (obj+0xb94) and the game-over flow are not modelled; voices and vibration are not played.
- `plyr` values (100 / 10 / 16) are constants taken from Sora's record, not read at run time.

## Verdict / next

PROGRESS: Sora reacts to hits by the exe's rules. Next: battle items (Items submenu, motion 9, heal formulas), then EXP and prizes, then Drive.
