# T069 — Guard

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T068. User request: keep rewriting the entire combat system.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence:
- Spec: `research/playable-room-20261008/guard-01/guard-spec.md`.
- Ghidra outputs: `research/ghidra-kh2-a3/guard-01/`.
- Trigger dumps and the ATKP rows: `guard-01/*.txt`.

## Findings

1. **Starting Guard.** Square runs PTYA 0x0b (Sora record 31): motion 173 (slot 692), ground only, needing ability bit 1 (item 0x52 "Guard") (FUN_1403a81f0 → FUN_1403a9c70 → FUN_140402750).
   - Frame trigger 21 turns Sora toward his target at tick 0.
   - Range id 2 makes him actionable at tick 52; the motion ends at tick 60.
2. **The guard is an attack.** It is not a flag.
   - W_EX010 slot 692 range 10 [6, 37] creates ATKP 161 (Type 2) on the Keyblade's group-2 spheres.
   - Guard types never test bodies.
3. **Clash (FUN_1403d08e0).** It runs before the body pass. The symmetric type table at DAT_1405c5e98 allows 0-0, 0-2, 0-3 and 1-3.
   - Each attack's ReflectRange cone must pass (FUN_1403d1f40: dot(v, f) < −cos R). The guard has R = 120°; the claws have 0, so no test.
4. **Guard success (FUN_1403d61d0, ground, RefactSelf 1).**
   - Sora: 00effect element 11 at the guard sphere's surface toward the claw. He faces the attacker and knock-slides away at 200 × 0.1 (×0.9 per tick, stops below 1), plays recoil motion 42, and gets 80 ticks without damage. The claw is recorded, so it does no damage.
   - RefactSelf/RefactOther 2 takes the pass path instead (effect 13).
5. **The Shadow (FUN_140411060 → FUN_1403d4650).** It plays the ReflectMotion (140 for ATKP 9/10, 141 for ATKP 11). It knock-slides back from its facing at (−1 → 50) × 0.1, its acts are cleared, and `mode_revenge` starts.
6. **Hit-stun.** Hit-stun holds acts below priority 300 while motion 10–39 or 140–150 *plays*. When the motion ends the act starts, so the Shadow no longer stays stunned after a finished damage motion.

## Implementation

- `battle_tables::Attack` gains RefactSelf/Other, ReflectMotion/HitBack and ReflectRange/Angle.
- `player_sim`:
  - `Action::Guard` and `Action::GuardRecoil`, with a lock until each clip's range-2 tick.
  - The turn to the target at the start.
  - `Player::guard_recoil` with the knock-slide.
  - `Input.guard` and `Input.target`.
  - A test covers the turn, the lock, and the knock-slide distance.
- `shadow`: `Shadow::reflect` (reflect motion, knock-slide, act clear, then mode_revenge), plus the reflect motions in the slot list and the hit-stun fix.
- `test_room`:
  - Guard is bound to F and the pad's west button.
  - It loads motions 173/42 for Sora and the Keyblade.
  - Keyblade guard windows feed a clash pass before the claws' body pass.
  - Sora's invulnerability timer.
  - System (00effect) effects with the PAX element's sound played as its request id.
  - The unattended run presses Guard at ticks 1170 and 1340 and captures tick 1198.

## Evidence (private, outside Git)

`autoplay-39` / `autoplay-40` (config `test-room-arena-04.json`):
- Guard starts at 1170.
- At 1193, claw ATKP 9 clashes with guard ATKP 161, with outcome "guard". Effect 11 and sound 108 play.
- The Shadow plays reflect motion 140, enters mode_revenge, and claws again at 1273, after its 80-tick reflect motion.
- Sora takes no damage from the guarded claw.
- The capture `autoplay-40/test-room-15.png` shows the guard flash, Sora's recoil and the Shadow's reflect.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS 180-frame smoke; PASS autoplay-39/40.

NOT RUN: an interactive playtest; a comparison with the installed game.

## Uncertainty

- **Inferred or test-room choices:**
  - Guard is treated as equipped (where the game grants it is unresolved).
  - How a Square press reaches the request flag is untraced.
  - The PAX element sound plays as a plain request id.
  - The bounce path (motion 41) is not loaded and is only logged.
  - The ReflectAngle rotation sign is assumed.
  - Sora's `stats+0x1c8` is taken as 0.
  - Range endpoint inclusivity is still open (T055).
- **Not implemented:**
  - The air guard and air reflect.
  - Counterguard and the other Square abilities.
  - Sora's own combo clashing with claws (Type 0-0). The clash table allows it, but this room only clashes guard windows.

## Verdict / next

PROGRESS: Guard blocks the Shadow's claws with the original's rules, reaction motions and effect. Next: the dialogue system with Axel as a test NPC (research running).
