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
| Command menu decide | se000 #2 (input bit 0x04, same functions) | `test_room.rs` plays it when a command runs | PLAYS (0a3f828) |
| Command menu back / close | se000 #4 | `test_room.rs` plays it when a submenu closes | PLAYS (0a3f828); open-submenu #3 still unmapped |
| Command menu unresolved labels | se000 #5 (error?), #24 (cancel?), #7 | none | UNRESEARCHED |
| Low-HP alarm (looped) | se000 #9, looped | `test_room.rs` SE_LOW_HP | PLAYS (not heard; autoplay HP never fell to the threshold) |
| Magic denied | se000 #5 | `magic_room.rs` | PLAYS |
| Item denied | se000 #5 | `item_room.rs` | PLAYS |
| Drive start, revert, antiform, form level | se000 #114, #115, #132, #26 | `drive_room.rs`, `drive_fx.rs` (on main at 3b7c738) | PLAYS (drive thread) |
| Party hit | per-hit id | `party_room.rs` | PLAYS |
| Keyblade swings and Sora hits | bank 500 (ATKP HitSound; W_EX010 trigger 8) | `test_room.rs` | PLAYS (combat thread, do not rework) |
| Shadow claws and jump | bank 201 (201020 claw, 201021 jump) | `test_room.rs` | PLAYS (combat thread) |
| Guard and deflect | 108 (PAX element sound), 500010 | `test_room.rs` (T069) | PLAYS (combat thread) |
| Cutscene events | 0x2B PlaySound with bank and number from 0x2A | `cutscene.rs` | PLAYS (cutscene only) |
| Walking footsteps | trigger 2 → bank from the room's 12soundinfo row, id = bank·1000 + p0 + material·16 (sound-01 §2.5, footstep-01) | `footstep.rs`, `test_room.rs` (T116) | PLAYS (not heard) |
| Sora battle voice | motion trigger 14, voice banks `voice/us/battle/...` | `test_room.rs` plays voices from the battle voice bank (on main at 3b7c738) | PLAYS (drive thread, not reworked) |
| Shadow appear and sink | effect-driven 201016, 201017, 201019 (PAX type 18) | only through the PAX element path above, if the Shadow's effects use them; not traced | UNRESEARCHED (combat and effects thread) |
| PAX effect sounds in general | the effect's sound field (`package_reader/src/effect.rs`) | `test_room.rs` plays `element.sound` as a plain request id (marked inferred in code) | PARTIAL (combat and effects thread): the id rule is not confirmed against the original |
| Field talk message window | per-character talk sounds (T070) | `test_room.rs` talk queue | PLAYS (not heard) |
| Doors and field effects | none: the original has no door or room-transition sound (fieldsfx-01) | nothing to add | N/A |
| Magic casting and release | effect-element sounds of the spell's script, cast at motion start and release at the release frame (magicsfx-01) | `spell_sound.rs`, `magic_room.rs` (T117) | PARTIAL: cast and release; hit sounds, Magnet's hold and end, Cure on others, Thunder's extra bolts and Final-form Blizzard volleys missing |
| Item use | motion 9 frame 10 plays se000 #305; the item effect adds #100 at the consume trigger and #101–104 after 30 ticks (magicsfx-01) | #305 plays from Sora's motion trigger 8 (T116); #100–104 need the item effect run | PARTIAL |
| Menu decide and back in the camp item and stock lists | se000 #2 and #4 | camp pick sounds | PLAYS |

## Gaps to close, in order

1. **Done (0a3f828):** command-menu decide and close. Open-submenu sound #3 is not played yet; its trigger is inferred, not confirmed.
2. **Done (T116):** footsteps. Material is polygon byte 0 of the floor under Sora.
3. **Done as far as the research goes (T117):** the room ambient bed. Doors have no sound. Chests, save points, prize orbs and event fades are not in the rewrite yet.
4. **Partly done (T117):** spell cast and release sounds. Hit sounds and the item effect's #100–104 remain.
5. **Combat list for routing** (not implemented here; Sora battle voice and Drive sounds already on main): Shadow appear and sink (201016/17/19), and general PAX effect sounds.

## Checks

NOT RUN for code: this change is documentation only, so no code checks changed. The exe rebuild with `--features skate3` was not run because no source changed from main.

## Uncertainty

- Every "PLAYS" row is a code-path claim, not a listening match.
- The command menu's unresolved labels (5, 7, 24) are not mapped.
- The 1:1 requirement is unmet for footsteps, doors and field effects, magic casting, and item use.

## Verdict / next

PROGRESS: audit complete for the sources in sound-01. Command-menu decide and close (0a3f828), footsteps and Sora's own motion sounds (T116), the room ambient bed and spell cast and release sounds (T117) now play. Still missing: spell hit sounds, the item effect's sounds, the combat items routed to the combat thread, and listening verification.
