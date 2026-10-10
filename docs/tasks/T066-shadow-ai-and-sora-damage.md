# T066 — Shadow AI, claw attacks and damage to Sora

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T065 (`194749d`). User request: keep rewriting the entire combat system.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence:
- The Shadow's AI program (`m_ex020`, BAR type 3 in `obj/M_EX020.mdlx`) was decoded with a private disassembler.
- The engine side (interpreter FUN_14041b400, act manager FUN_1403cae60/FUN_1403cab00/FUN_1403ca550, hit-stun FUN_1403ca7c0, trap tables at 0x140753490) was read in Ghidra.
- Spec: `research/playable-room-20261008/ai-01/shadow-ai-spec.md`.
- Combat data: `combat-01/combat-spec.md`.

## Findings

1. **Acts and scheduling.** Acts are named threads with priorities (claws 200; leave/dead 300, locked).
   - `act_start` clears the queue and runs in the same tick.
   - Acts below 300 pause while a damage motion (10–39) plays.
   - OBJ_MAIN runs `appear_shadow` then `mode_battle`. A reacting hit by the player starts `mode_revenge`. HP 0 starts `dead`.
2. **mode_battle choices by distance.** Under 300: btl_attack. Under 600: btl_short. Otherwise btl_long.
   - btl_attack: 50% slither; otherwise claw L under 150, or 30% slither / 60% claw R / 10% jump claw at 150–350. battle_idle (footwork 60–90 ticks) follows every branch.
   - btl_short: walk/idle mixes.
   - btl_long: slither, with a 20% leave check.
   - Revenge: under 400, 30% jump claw and 70% claw R.
3. **Movement.**
   - Slither (sink 161 → two legs → rise 162): the legs go to a random point 200–300 away and then to the target. Each leg lasts 75–120 ticks or ends within 100 units. Speed is min(dist, 14) per tick, with a ±60°·sin(phase) weave advancing 3–5° per tick.
   - Walking is root motion (166.67 units per 40 ticks).
   - Jump claw: height 100 and speed 5, with gravity constant 0.408163.
4. **Random numbers.** One script LCG (×69069, seed 1); `random_getf` = state·2⁻³²·x.
5. **Claws.** Windows come from range trigger 10 in M_EX020.mset: 201 at ticks 33–46 (ATKP 9, group 1), 202 at 53–65 (ATKP 10, group 2), 203 at 22–36 (ATKP 11, group 3). Spheres are the Shadow's ATTACK collision entries.
   - ATKP 9–11 rows exist only at sub-table 0: Power 65, reaction 3, KB1 0x7fff (→ SSTM HitBackSmall 200).
   - Damage to Sora: the Shadow's LVPM Str 5 clamped to max 3, minus Sora's Def 2 → 2 per hit.

## Implementation

- `player_sim::shadow`: the AI translated as an act state machine (all acts listed above), the engine reactions (flinch/blow, landing), the script RNG and root-motion movement. Tests cover the RNG, appearance into combat, and death.
- `player_sim`: `Action::Hurt` with front/back × high/low clips (motion ids 10–13) and a decaying knock-back slide. `combat` keeps the shared damage and shape helpers.
- `test_room`:
  - The Shadow runs its AI.
  - Its claw windows are swept against Sora's HIT spheres (P_EX100 collision).
  - Hits damage Sora's HP, which now drives the HUD, and play his hurt motion.
  - A KO refills HP after 2 s (test-room choice). G refills HP.
- The unattended run is 1800 ticks with captures at 1300/1500/1700, so the Shadow attacks a standing Sora.

## Evidence (private, outside Git)

`research/playable-room-20261008/autoplay-34` (log and captures). In it:
- The Shadow appears, claws (ClawR at tick 68), idles with footwork and slithers.
- It dies to the ground combo (3/2/6), then reappears and resumes.
- It hits Sora at ticks 1194 (claw L, 2 dmg), 1361 (jump claw, 2) and 1671 (claw L, 2). HP goes 20 → 18 → 16 → 14 and the hurt motions play.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (167 tests); PASS build; PASS 180-frame smoke; PASS autoplay-34; PASS release `test_room` build. NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

Test-room choices / inferred:
- The default turn rate (0.13 ×2 rad per tick; sysobj+0x134 untraced).
- Motion-length units (60 Hz ticks assumed).
- The movement integrator. The weave's rotation sign is inferred.
- The slide and blow brakes, and fades shown as hide/show at half alpha.
- Hits ignored while sunk.

Not implemented: step/ledge jumps in walking, the leave-when-off-screen distance (1200), hit effects and sounds, Sora's air damage reactions, and Sora's death or game over.

## Verdict / next

PROGRESS: the test arena now has a Shadow that fights with the original's decision logic and damages Sora. Next: hit effects and sounds, Sora's guard/dodge commands, and the remaining physics constants.
