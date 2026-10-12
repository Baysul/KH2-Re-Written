# T115 — Sound effect audit: what the original plays and what the rewrite plays

Owner Claude, 2026-10-12. Gate: P22 (front-end and field scope), audit only. User request (relayed by the coordinator): add all sound effects, walking through magic and battle, 1:1 with the original. This record is the first step: the audit. Combat files are not reworked (the "Drive forms and growth abilities" thread owns hit, voice, and swing effects); combat gaps are listed for routing.

## Evidence

- Private sound research: `research/playable-room-20261008/sound-01/sound-spec.md` (installed build, bank and id mapping, SCD/MS-ADPCM decode). Its tags: [O] decompiled, [D] decoded from installed data, [I] inferred.
- Event opcodes 0x2A (SoundData) and 0x2B (PlaySound): `crates/package_reader/src/event.rs`.
- Rewrite call sites, read from the working tree at 6c5fe1e.

Nothing was listened to or compared against the installed game. Every "plays" below means the code path requests the id, not that the output matches the original.

## Audit

Status key: **PLAYS** the rewrite requests the id on the original's trigger (not listening-verified); **PARTIAL** some triggers only; **MISSING** the original's trigger has evidence but the rewrite has no path; **UNRESEARCHED** the original plays it, but the trigger or id is not yet traced.

| Category | Original (sound-01) | Rewrite now | Status |
|---|---|---|---|
| Camp and pause menus: open, move, decide, cancel, invalid | se000 #3, #1, #2, #4, #5 (sound-01 §2.2) | `camp.rs` `pick_sound`, camp submodules | PLAYS |
| Title and New Game menus | se000 #1, #2, #4, #5 (T071, T072) | title binary | PLAYS |
| Command menu cursor | se000 #1 (input bits 0x10/0x20) | `test_room.rs` SE_CURSOR | PLAYS |
| Command menu decide | se000 #2 (input bit 0x04, same functions) | none | **MISSING** (menus) |
| Command menu back / close | se000 #4 | none | **MISSING** (menus) |
| Command menu unresolved labels | se000 #5 (error?), #24 (cancel?), #7 | none | UNRESEARCHED |
| Low-HP alarm (looped) | se000 #9, looped | `test_room.rs` SE_LOW_HP | PLAYS (not heard; autoplay HP never fell to the threshold) |
| Magic denied | se000 #5 | `magic_room.rs` | PLAYS |
| Item denied | se000 #5 | `item_room.rs` | PLAYS |
| Drive start, revert, antiform, form level | se000 #114, #115, #132, #26 | `drive_room.rs`, `drive_fx.rs` | PLAYS (drive thread) |
| Party hit | per-hit id | `party_room.rs` | PLAYS |
| Keyblade swings and Sora hits | bank 500 (ATKP HitSound; W_EX010 trigger 8) | `test_room.rs` | PLAYS (combat thread, do not rework) |
| Shadow claws and jump | bank 201 (201020 claw, 201021 jump) | `test_room.rs` | PLAYS (combat thread) |
| Guard and deflect | 108 (PAX element sound), 500010 | `test_room.rs` (T069) | PLAYS (combat thread) |
| Cutscene events | 0x2B PlaySound with bank and number from 0x2A | `cutscene.rs` | PLAYS (cutscene only) |
| **Walking footsteps** | trigger 2 → bank from the room's 12soundinfo row, id = bank·1000 + p0 + material·16, p0 alternates by a counter (§2.5) | none | **MISSING**: the floor-material value (an actor field set from collision attributes) is unresolved, so no id can be chosen 1:1 |
| Sora battle voice | motion trigger 14, voice banks `voice/us/battle/...` | none | **MISSING** (combat thread) |
| Shadow appear and sink | effect-driven 201016, 201017, 201019 (PAX type 18) | only through the PAX element path above, if the Shadow's effects use them; not traced | UNRESEARCHED (combat and effects thread) |
| PAX effect sounds in general | the effect's sound field (`package_reader/src/effect.rs`) | `test_room.rs` plays `element.sound` as a plain request id (marked inferred in code) | PARTIAL (combat and effects thread): the id rule is not confirmed against the original |
| Field talk message window | per-character talk sounds (T070) | `test_room.rs` talk queue | PLAYS (not heard) |
| **Doors and field effects** | field event PlaySound 0x2B, which needs the field's event scripts to run | field event playback does not run outside cutscenes | **MISSING**: no field script executes in the rewrite; which events hold door and field sounds is not enumerated |
| **Magic casting** | the cast motion's trigger and the spell's cast sound | cast motion runs (`magic_room.rs`), no sound | **UNRESEARCHED**: the sound source is not traced |
| **Item use** | camp decide sound only (T079); item-use effect sound not traced | camp decide only | UNRESEARCHED |
| Menu decide and back in the camp item and stock lists | se000 #2 and #4 | camp pick sounds | PLAYS |

## Gaps to close, in order

1. **Command menu decide and back** (`se000` #2 and #4): the call sites are known (sound-01 §2.2). Small and menu-scope; blocked only on writing the command-menu hook. Not done here: the command menu is HUD code shared with combat.
2. **Footsteps:** the timing and bank formula are known; the floor-material mapping (an actor field set from collision attributes) is not. Needs its own research task before any id can match the original.
3. **Field event sounds (doors and field effects):** enumerate every 0x2B and 0x2A command across the installed field event scripts, then decide which play in the rewrite's rooms. This requires a field event player, which is a separate gate.
4. **Magic casting and item-use sounds:** trace the cast and item-use sound sources (the cast motion trigger and the spell or item effect records).
5. **Combat list for routing** (not implemented here): Sora battle voice (trigger 14), Shadow appear and sink (201016/17/19), and general PAX effect sounds.

## Checks

NOT RUN for code: this change is documentation only, so no code checks changed. The exe rebuild with `--features skate3` was not run because no source changed from main.

## Uncertainty

- Every "PLAYS" row is a code-path claim, not a listening match.
- The command menu's unresolved labels (5, 7, 24) are not mapped.
- The 1:1 requirement is unmet for footsteps, doors and field effects, magic casting, and item use.

## Verdict / next

PROGRESS: audit complete for the sources in sound-01. Missing and unresearched sounds are listed above, with the combat items routed to the combat thread. Next: command-menu decide and back (step 1), then the footstep material research (step 2).
