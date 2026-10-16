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

## Checks

- PASS `cargo fmt --all --check`, PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  PASS `cargo test --workspace --locked` (new: 7 in `player::magic` for cast effects, Fire/Blizzard/Thunder/Reflect
  effect calls, Final Blizzard volleys and Reflect's replaced barrier; 1 in `player` for several releases and one
  MP charge; 3 in `spell_fx` for placement, bind modes and direction), PASS `cargo build --workspace --locked`.
- PASS release `--magic-tour` runs (base form, `--magic-form 2` and `5`): every spell starts its effect sequence,
  element sounds and cast voice; casts carry the form's row (14 and 34); no effect program is reported unsupported
  except pppSRandUpFV (76) in Thunder's hit spark.
- Compared with the executable (static): the combo gate `FUN_1403d6ca0` / `FUN_1403d6fc0` / `FUN_1403d7780` /
  `FUN_1403a9dd0`, the cast start `FUN_1403c5170`, the effect start and bind routines, and the window data the
  gate reads (the installed P_EX100 motions' range triggers, logged as `motion_windows`: motion 151's combo
  window is frames 23–48, and a cast pressed at frame 21 is refused, at 26 accepted).
- Compared only in the test room (no capture or run of the installed game): effect placement, timing and look,
  the finisher sequence, chaining after a cast, form casts.
- NOT RUN: owner playtest; frame-by-frame comparison with the installed game; the Master form's tour.

## Uncertainty

- Bone 0x4003 (the Keyblade's top) is the farthest vertex of the posed Keyblade, not
  the weapon's WEAPON_TOP collision entry with its offset (`FUN_1403d7600`). Bone 0x4001
  and 0x4009 use the TARGET shape's lift, not the type-1 and type-9 collision entries.
- pppKeZCrct mode 3's cross-product order is inferred.
- Effect fade-out frames are treated as 30 Hz frames (2 ticks); unverified against a capture.
- Magnet's sphere scale (the sin ease) assumes `effect_set_scale` sets the whole scale
  vector.
- Not modelled: Magic Lock-On (ability 115), `bg_get_random_pos_air` for Final Thunder
  (a fixed ring is used), Cure's target menu and party members' casts, spell hit sounds
  other than the element sounds, and which party slot Cure heals.
- Which behavior the owner saw as "magic does not use the combo system" was not
  reproduced; see the report in the project thread.

## Verdict / next

PARTIAL: the spells show their own effects, animations and variants and join the combo as
the executable's gate does. Next: pppSRandUpFV for Thunder's spark, the exact WEAPON_TOP
and collision-entry bones, Magic Lock-On, owner playtest of each spell in each form.
