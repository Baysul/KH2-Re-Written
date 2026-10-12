# T117 — Room ambient bed and spell cast and release sounds

Owner Claude, 2026-10-12. Gate: P22 (field scope). Starts from `main` at da613bd and T116. User request: add every sound effect, walking through magic and battle, 1:1 with the original. Build: Steam build 15194255 through the session-validated source (`source-manifest-02`). Drive and combat files are not reworked.

## Evidence

Private research, no game bytes in Git: `fieldsfx-01` (ambient bed rule, no door sound), `magicsfx-01` (spell sound tables, effect-sound rule).

## Changes

- **Ambient bed** (`footstep::WorldInfo`, test room): the room's 12soundinfo row names a bank and a sound number (number -1 means none). That sound plays looped, as a plain request id. When the room changes and the bed changes, the old bed fades out over two seconds while the new one starts at once; an unchanged bed continues.
- **Large banks:** the ambient bank is larger than the reader's 32 MiB inspection limit, so `Library::read_limited` and `original::read_original_limited` read an entry up to an explicit cap (256 MiB here). The default limit and every other caller are unchanged.
- **Spell sounds** (`windows_shell::spell_sound`, magic room): the cast sound plays at the cast motion's start (Fire 116, Blizzard 120, Thunder 126, Cure 129, Magnet 133; Reflect has none, and Fire's is skipped in Wisdom and Final). The release sounds play at the release frame by level: Fire 117/140/156 with a second ball 999016 in Master and in the Wisdom and Final finishers, and 141/142/146 for Final's finisher; Blizzard 121/143/159; Thunder 127/148/164 for one bolt; Cure 130/150/166 on the target; Magnet's sphere 134; Reflect 138/175/177.
- A sound-bank load that fails is reported on the console instead of ignored.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (new tests: ambient row reading, spell sound tables); PASS build; PASS unattended Twilight Town room (tt08, 900 and 1500 frames).

- Bank 3 loaded and request 3004 resolved to three looped layers at volume 0.71; room start time was unchanged at about 12 s.
- The run cast five spells and released six, each requesting the ids in the table above.
- NOT RUN: listening comparison against the installed game.

## Uncertainty

- Reflect's 138/175/177 are read as the three levels of the same element; the research table lists them under element variants as well. Not confirmed.
- Thunder plays one bolt sound per release. Master (a ring of bolts plus the centre) and Final (a loop of bolts) have more, on their own schedule.
- Blizzard's Final-form finisher alternates two elements across five volleys; one sound per release is played.
- Cure's second sound for other party members in range and every spell's hit sounds are not played. Magnet's hold loop (135/153/169) and end (137) are not played.
- The ambient bed decodes on the main thread when the room starts.
- Effect sounds wait for the element's start time in the original; spell sounds here play at the cast and release moments.
- The bed's 3D-rate, bus and attenuation settings are not modelled.

## Verdict / next

PROGRESS: the room ambient bed plays with its cross-fade, and each spell requests its cast and release sounds. Next: spell hit sounds and Magnet's hold, then the item effect's sounds (#100–104).
