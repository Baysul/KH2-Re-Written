# T116 — Footsteps, movement sounds and item-use sounds

Owner Claude, 2026-10-12. Gate: P22 (field scope). Starts from `main` at a7a8a47 and T115. User request: add every sound effect, walking through magic and battle, 1:1 with the original. Build: Steam build 15194255 through the session-validated source (`source-manifest-02`). Combat files (hit, voice and swing effects) are not reworked.

## Evidence

Private research, no game bytes in Git:

- `footstep-01/footstep-spec.md`: the footstep rule (floor material, bank, counter).
- `magicsfx-01/magicsfx-spec.md`: magic casting, item use, menu sounds and the effect-sound rule.
- `fieldsfx-01/fieldsfx-spec.md`: field event sounds, ambient beds, object sounds.

## Changes

- **Footsteps** (`windows_shell::footstep`, test room): Sora's motion trigger 2 plays `bank * 1000 + p0 + material * 16`.
  - The material is polygon byte 0 of the floor under Sora (`MapWorld::floor_ground`, from `field_collision::FloorInfo::ground`); 0 plays nothing.
  - The bank is the room's 12soundinfo Sora entry. Every Sora bank of the world is loaded at room start.
  - p0 0 and 4 pick one of four variants and p0 13 one of two, by one counter that advances only on those steps and only when a sound can play.
- **Movement sounds:** Sora's own motion trigger 8 plays `p0 * 1000 + p1` (cloth 300–302, jump 303, item use 305 and the others in the actor's motions).
- **Item use:** a refused use (state gate) is silent now. The "cannot" sound (se000 #5) plays only on deciding a dimmed row, which the command menu already did.
- **Effect sounds:** an effect element's sound plays only when its id is above 0.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (seven new footstep tests: bank by room, short data, run/walk variants, jump and land, no material, no bank, p0 13); PASS build; PASS 180-frame smoke; PASS unattended Twilight Town room (tt08, 2400 frames).

- The unattended run logged 22 footsteps (bank 688, material 1: run 688016–19, walk 688020–23, jump 688025, land 688026) and eight movement sounds (se000 #300–303), all resolved to a loaded sound.
- NOT RUN: listening comparison against the installed game.

## Uncertainty

- The gate on the actor's flag bit 0 that precedes trigger 8 is not modelled; its clearing point is unresolved (fieldsfx-01). If it is clear in some states, those states would be silent in the original.
- Trigger scans use the clip's frame range per tick, so two steps can request in one tick at run speed; the original's per-tick scan is not observed.
- Only the starting world's Sora banks are loaded. After a world warp, footsteps are silent until the room restarts.
- Material 0x12 in three physics modes (10, 20, 21) is not modelled; the test room does not enter them.
- Which motion variant (slot % 4) plays per Drive form is unresolved.

## Not done, with the data now in hand

- **Spell sounds:** casts and releases are effect-element sounds of the `magic/*.mag` effects, started by the cast and release triggers. The rewrite's magic room does not run those effects, so no spell plays a sound. Needs the magic effect run, then the element sound rule above (wait for the element's start, positional).
- **Ambient bed per room** (12soundinfo row bank and number, looped, 2 s cross-fade): not implemented.
- **Doors:** the original has no door or room-transition sound (fieldsfx-01); nothing to add.
- **Object sounds** (chests, save points, prize orbs, treasure windows): those objects are not in the rewrite.
- **Event sounds with a non-zero mode** (fade a slot's sound over one second): 62 records, not handled by the cutscene player.
- **Effect-sound timing:** the original waits for the element's start-wait (plus 0.1 s when bound) before the sound; the Drive effects path still plays at once and tests `>= 0` (`drive_fx.rs`, Drive thread).

## Verdict / next

PROGRESS: Sora's footsteps and Sora's own motion sounds now play from the installed banks, and a refused item use is silent. Next: the magic effects run (spell sounds), then the room ambient bed.
