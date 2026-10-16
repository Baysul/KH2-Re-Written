# T136 — Magic: the spells' own effects, form variants and the combo check

Owner Claude, 2026-10-16. Gates P18/P21 (test-room scope). Starts from `main` at
`1e60690` (package 0.1.2) and T092, T117, T121. User request: fix the magic system
to match the original 1:1 — spells go through the combo system and have their own
effects and animations. Selected KH2FM Steam build 15194255, validated at every
launch; the installation stays read-only. Branch `claude/project-thread-8zaost`.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/magic-02/`
(`magic-effects-spec.md`, Ghidra decompiles `g/d01`–`g/d05`, table dumps `g/t01`–`g/t04.txt`,
tour logs and captures in `runs/`). Nothing from the game is in Git: the effect files
(`magic/*.mag`) and `effect/texcommon.dpd` are read from the installation at run time.

## Steps

1. Ran the unmodified 0.1.2 room unattended: casts already played the cast and release
   motions, spent MP and counted as a combo hit, but every spell showed a translucent
   placeholder shape instead of its effects, and a cast ignored the Drive form.
2. Decoded from the executable how a script starts and steers an effect element
   (`FUN_1402c6b30`, `FUN_1402c5f70`, `FUN_1402c49d0`, the `effect_*` traps
   `0x14041df80`–`0x14041e2c0`, bone codes `FUN_1403b50f0`).
3. Translated that into `player::magic` (the six spell scripts now issue
   `pax_start` / `pax_start_bind` / `effect_*` calls in script order, with the cast
   effect at trigger 20, the release effects at trigger 21, shard, bolt, ring,
   barrier, flash and counter effects, Reflect's shared barrier block, Final Form's
   repeated Blizzard releases and the per-form variant rows) and `test_room/spell_fx.rs`
   (places, steps and draws the elements; element sounds play at the element's start).
4. Loaded each level's `magic/*.mag` effect container; a hit on the enemy starts the
   element of the spell's own container chosen by the ATKP hit-effect category.
5. Added four effect-VM programs the spell files use: pppDrawMdlLoop (218),
   pppMatrixLoopYXZ (232), pppVertexApLcLoop (236) and pppKeZCrct mode 3.
6. Passed the Drive form (Wisdom +10, Master +20, Final +30) into the cast.
7. Added `--magic-tour <dir>` (with `--magic-form <n>`): an unattended run of the six
   spells on the ground and in the air, combo probes and form casts, with captures.
8. Removed `visual::spell_sound`: the cast and release sounds now come from the effect
   elements themselves, as the executable plays them.
9. Cure's target menu: the Magic list's Cure row opens the party list (Sora, Donald, Goofy);
   the pick is the heal target. The heal point is the member's type-9 centre; members within
   Cure's range heal too (708 for the caster's own pick, 709 for the others).
10. Donald's four spells (`player::party_members` records 57-60 and `player::party_spell`,
    translated from the acts of his BDX `p_ex020`: `fire` L2003, `blizzard` L2571 with the
    shot thread L2767-L4992, `thunder` L5052, `cure` L5453): the think loop's random
    MP-scaled spell permit and the MP gate, the cast motions 191/192 and their release
    (frame trigger 9, ticks 20 and 26), MP spent at the release (Cure's 255 always starts
    the 1200-tick MP Charge), effect elements from `obj/P_EX020.a.us` bound to his staff
    top, volumes with Donald's Magic stat, and the heal on the chosen member
    (`party_room.rs`, `spell_fx.rs`).
11. Found why Magic Lock-On had no candidates: the legacy enemy's per-tick bone matrices
    held only its HIT and claw bones, so its TARGET shapes were never placed. The bones of
    the TARGET shapes are now included; the lock-on and Donald's Thunder read them.
12. `--magic-tour` gained Donald's four ordered casts, the party AI running free, and two
    Magic Lock-On probes (enemy 1400 ahead, ability on and off).

## Findings

- Combo gate: a cast is accepted only inside a combo or finisher window or when the
  motion takes commands (`FUN_1403ba070`), counts as one combo hit
  (`FUN_1403d6ca0(obj, 1)`), and is the finisher when the counted step reaches the
  limit. Measured with an enemy in front: after Attack 1 a cast is refused at frame 21
  and accepted at 26 (the motion's combo window starts at 23); Attack, Attack, Fire plays the finisher motion 57 and
  row 5; a cast that hits lets the next Attack chain. The unmodified build already
  did this; no gap was found in the player-side combo logic.
- Casts in Final Form use that form's motion set and rows (34, 35, ...) and release
  Blizzard once per volley.
- Donald's acts (static, from the BDX): Fire starts element 0 at the release and 4 ticks later
  element 1, whose position drives an ATKP 1163 ellipsoid (220 x 70) for up to 60 ticks;
  Thunder starts element 7, waits 10 ticks, then element 8 (floor within 160 below the
  target) or 9 at the target (or 300 ahead) with ATKP 1165 as a sphere (160) and a column
  (25 x 320, centre 320 above), 20 ticks; Blizzard starts element 3, then a shot (element 4,
  ATKP 1164, sphere 80) that never turns (its turn rate stays 0), moves 60 per tick, fades
  after 30 ticks and its attack ends 8 ticks later; Cure starts elements 11 (caster) and 12
  (on the target), spends 255 and strikes ATKP 1525 on the chosen member.
- Cure's AI target (`FUN_1401cc1a0`): the partner in need (Sora, else the other member, at
  a quarter of the HP), else itself, Sora, the other member, each under 0.4 of the maximum HP.

## Checks

- PASS `cargo fmt --all --check`, PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  PASS `cargo test --workspace --locked` (new: 7 in `player::magic` for cast effects, Fire/Blizzard/Thunder/Reflect
  effect calls, Final Blizzard volleys and Reflect's replaced barrier; 1 in `player` for several releases and one
  MP charge; 3 in `spell_fx` for placement, bind modes and direction), PASS `cargo build --workspace --locked`.
- PASS release `--magic-tour` runs (base form, `--magic-form 2` and `5`): every spell starts its effect sequence,
  element sounds and cast voice; casts carry the form's row (14 and 34); no effect program is reported unsupported
  (pppSRandUpFV, Thunder's hit spark, arrived from main).
- Compared with the executable (static): the combo gate `FUN_1403d6ca0` / `FUN_1403d6fc0` / `FUN_1403d7780` /
  `FUN_1403a9dd0`, the cast start `FUN_1403c5170`, the effect start and bind routines, and the window data the
  gate reads (the installed P_EX100 motions' range triggers, logged as `motion_windows`: motion 151's combo
  window is frames 23–48, and a cast pressed at frame 21 is refused, at 26 accepted).
- Compared only in the test room (no capture or run of the installed game): effect placement, timing and look,
  the finisher sequence, chaining after a cast, form casts.
- PASS release `--magic-tour --magic-form 4` (Master rows 24 and 25 cast with effects); the second pass adds the exact Keyblade tip, body-centre and heal-point bones, Final Thunder's position draw and Magic Lock-On.
- PASS release `--magic-tour` (third pass, base form and `--magic-form 5`): Sora's 26 cast and refusal events up to tick 4600
  are identical to the earlier base run (one probe's attack index differs in the Final run: 21 against 6,
  after the merge of main, not investigated). Cure on Donald and on Goofy heals the picked member with ATKP 708 and the
  others with 709. Donald's ordered Fire, Thunder, Blizzard and Cure start their effects (elements 0, 1, 7, 8, 3, 4, 11, 12,
  none reported missing or unsupported), release 26, 36 (10 after the release), 20 and 26 ticks after the cast starts, spend
  10, 10, 10 and 255 MP (Cure starts the charge), and land ATKP 1163 (three hits at the 12-tick interval, the third on
  step 2), 1165 (sphere and column, every 8 ticks), 1164 (one hit) and 1525 (heal 10 on Goofy). With the AI free, Donald
  chooses spells by the permit and stops while the MP Charge runs.
- PASS Magic Lock-On, tour probes at ticks 7500 and 7700 (enemy 1400 ahead, beyond the automatic range): with the ability
  the cast finds one candidate, locks it and Fire is released toward the enemy; without it the release has no target.
- NOT RUN: owner playtest; frame-by-frame comparison with the installed game.

## Uncertainty

- Keyblade tip (bone 0x4003): bone 2's origin plus its offset along the blade, taken as the posed vertex farthest from bone 2 (measured 77.96 against the 78-unit offset in the original's tables), not read from the collision entry. Bones 0x4001 and 0x4009 use Sora's own type-1 (85) and type-9 (105) collision entries; party members use the same lifts.
- pppKeZCrct mode 3's cross-product order is inferred.
- Effect fade-out frames are treated as 30 Hz frames (2 ticks); unverified against a capture.
- Magnet's sphere scale (the sin ease) assumes `effect_set_scale` sets the whole scale vector.
- Final Thunder's extra bolts use the original's draw (radius, eight-sector angle order, height) on the spell's own random stream, not the shared script stream, and without the background test.
- Donald's staff top (bone 0x4003/4) is taken the way the Keyblade tip is: bone 2 of his staff with the posed vertex farthest from it. His type-1 and type-9 collision centres are read from `P_EX020.mdlx`.
- Cure's record reads a range of 0-0 and flags 3; read literally the original's range test would only pass when the enemy overlaps Donald, so the cast waits on the heal target instead. The record's flags were not decoded.
- Donald casts with all four spells equipped (the test room does not model his equipment); the original needs each ability learned and equipped.
- Not modelled for the party: the voices (`?1,262`), hit sparks of the party's spells, reflect of Blizzard's shot, interrupting a cast, Goofy's spells, Donald's Limit and items, and Donald being hit.
- Not modelled: controller vibration on casts.
- Which behavior the owner saw as "magic does not use the combo system" was not reproduced.

## Verdict / next

PARTIAL: the spells show their own effects, animations and variants in the base, Wisdom, Master and Final forms and join the combo as the executable's gate does; Cure has its party target menu; Donald casts his four spells; Magic Lock-On works in the room. Open: owner playtest of each spell in each form, a comparison with the installed game, and the party items in the Uncertainty list.
