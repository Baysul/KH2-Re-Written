# T090 — Item pictures (keyblades, items, equipment)

Owner Claude, 2026-10-10. Gate P22. Starts from T088 (the Journal) and the open gaps of T079 / T085.

## Source

Private evidence: `research/playable-room-20261008/camp-05/items-spec.md` §3.6 and `camp-09/equip-spec.md` §2.3 / §6 (picture rules), and a new decompile set at `ghidra-kh2-a1/itempic-01..06` (FUN_1403b0c90, FUN_140301d80, FUN_140300dc0, FUN_1403018e0, FUN_1403022b0, FUN_1403022a0, FUN_140302370, FUN_140302040, FUN_140302200, FUN_140302280, FUN_140301cc0, FUN_140301c80, and the loader FUN_1403f4c30).

## What was missing

The camp drew no item pictures. The Items stock list, the Use / Throw away screen and every Equip screen have a picture panel, so keyblades, armor, accessories and items showed an empty box. T079 listed the sequence DAT_142a11888 as unresolved.

## Findings

- **The sequence is the `ipic` SEQD of 03system.bin** (BAR type 25, 6264 B, 7 animation groups). FUN_1403f4c30 reads it by name (`ipic`, FUN_1403a6480 type 0x19) and stores it with FUN_1403b0ca0 into DAT_142a11888. This resolves the open item in T079.
- **The image is `itempic/item-%03d.imd`**, a bare IMGD with its HD replacement, in `kh2_fifth.hed`. The id is the item record's +0x14 (`package_reader::item_table::Item::picture`).
- The picture sprite plays group 0 on show and group 1 on close; scale = the parent group's TextPositionX / 100 (0.6 in the stock frame G348, 1.0 on a holder).

## Implementation

**`package_reader::name_hash`:** MD5 of an archive path, the HED name hash. Test: RFC 1321 vectors, and the known hash of `menu/us/jmemo.2ld`.

**`layout2d::load_imgd`:** finds `itempic/item-NNN.imd` in the index by its path's hash and returns its HD image; None if the index lacks it. The source is validated as for every other read.

**`camp::picture`:** FUN_1403022b0 / 1403022a0 / 140302370 / 140302040 / 140302200 / 140302280 / 140301cc0 / 140301c80 / 140301d80:
- request → close what shows, hold 60 f, wait 15 f;
- then ask the loader for the image (`PictureCache.wanted`), and show it once it is in the cache;
- close plays group 1 and the picture dies after it;
- the stock list makes its own frame (G348 → 349, close 350, prio 0x753f); the command screen and Equip put the picture on their holder.

**Screens:**
- Items stock (state 10): requested with the help line; closes on move, tab change, decide and cancel; the screen waits for it to die.
- Use / Throw away (state 11): on the holder at 0x753c; closed when leaving.
- Equip slot list (state 2) and the pick lists (5–9): on the holder at 0x753f; the old picture closes on each move, and the new one comes after the timers. An empty slot or the "unequip" row has none.

**`test_room`:** the `ipic` sequence is read from 03system; pictures load from the install when the camp asks (`load_wanted_picture`), by the item's id, and are drawn after the sprites of their priority. The unattended walk rests longer on the Items and Equip screens so the 15-frame delay is captured.

## Evidence (private)

`autoplay-81` (arena-07 via `kh2fm room`, exit 0, 82 captures, none uniform):
- 63: the Equip slot list with the Kingdom Key picture beside Strength 3 [5], Magic 1 [7];
- 40: the Use / Throw away screen with the Ether picture on the holder;
- 66: the pick list with the Potion picture in its box;
- 64: an empty item slot shows no picture.

`autoplay-80` (the same build before the walk's rests were lengthened) passed too, with no picture shown in the captures taken before the 15-frame delay ran out, as the exe's timers give.

## Checks

PASS fmt; PASS clippy; PASS tests (new: MD5 and the HED path hash); PASS build; PASS smoke; PASS autoplay-81.

NOT RUN: comparison with installed-game captures; armor and accessory pictures (no such items in the room's stock); the stock list's own frame G348 pose (the room's stock list capture rests under 15 frames).

## Uncertainty

- **[I] Stock frame:** its picture at 0.6 uses G348 / 349 / 350 from the spec; no capture was compared.
- **Loading:** the exe loads asynchronously with a small cache (70 slots, FUN_140301390); here a load is immediate and every image stays loaded.
- **Not built still:**
  - keyboard key-cap prompts (`09 da..f9` while the keyboard was last used; Xbox icons are drawn instead);
  - the Journal's category pictures (character faces `jmface_<code>.2ld`, album photos `jm_photo`, puzzles `jm_puzzle`) with their pages;
  - form sub-weapons (state 6) and their pictures.

## Verdict / next

PROGRESS: the Items, Use and Equip screens show their pictures. Next: the Journal category pages with their pictures, or the key-cap prompts.
