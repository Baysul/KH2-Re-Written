# T079 — Camp Items screens, inline icons, text codes, font shader correction

Owner Claude, 2026-10-09. Gate P22. Starts from T078. User request: pause menu 1:1.

## Prerequisites / source

Private evidence:
- `research/playable-room-20261008/camp-05/items-spec.md` (Ghidra `ghidra-kh2-a1/items-01..02`, 03system item table);
- `text-codes-01/text-codes-spec.md` (Ghidra `ghidra-kh2-a3`, `text-codes-01/g/tc-01..12`).

## Implementation

**`package_reader::item_table`:** the 03system `item` table, 0x18-byte records in file order (FUN_1403e3960 / FUN_1403e39a0).

**`camp::items`:** states 1 and 10–14, translated from FUN_14034ff50 / df80 / 1403504e0, FUN_14034fa20 / dba0 / 1403509b0 / 14034ccc0, FUN_14034ef80 / d2d0, FUN_14034f5f0 / d640 and FUN_14034da70 / d120.

- **State 1, Items top:**
  - a row per member plus Stock;
  - the preview panel with 20 bars and texts: a member's equipment sections (weapon, forms, armor, accessories, items, each starting a new grid row), or the first 20 stock items;
  - help 0xbb86 / 0xbb87; the Stock NEW! badge.
- **State 10, stock grid:**
  - 18 rows on the frame (G293 → 294);
  - "×N" counts drawn by the row's callback text;
  - NEW! badges and the seen marks;
  - the scrollbar (FUN_1402fa930 / facd0);
  - the category tab bar with labels, L2/R2 arrows and highlight; R2/L2 switch tabs and rebuild;
  - the grid input of FUN_1402f9440 / f8650: 2×9, wrap on a fresh press, row-internal left/right, L1/R1 page jumps;
  - "There are no items." (G302) with no hand or LightCur;
  - the cursor fix-up when the list shrinks.
- **State 11, command menu:** item name and count, Use / Throw away (grey when it cannot be sold), the selection marker, the holder panel; the invalid row plays #5.
- **State 12, use target:** "Use on who?", a row per member or "All" (party-wide items), the party block in mode 1 (G104–106, 2-column member grid).
  - The HP/MP item validity and effect: ceil(max·a/100).
  - se #37 on use; one fewer in stock.
- **States 13 / 14, dialog boxes:**
  - the dialog box (G6 → 7 → 8) with its choice rows (G3/4/2/5) on the box, which RIGHT/LEFT move;
  - "will have no effect" (OK) and "Throw … away?" (No / Yes), built with the 0x0d substitution of FUN_1403ae030.
- **Hands:** earlier lists keep a dimmed hand (G23); the stock list uses G22.
- **Waits:** they skip on any press, as the exe does.

**`camp`:** the new states and routing; Items entry 0 from the top level; `MenuList.sideways` for the dialog choices; and `status_block_with` for the mode-1 block.

**`sprite2d` text (FUN_1403b0280 / FUN_140406ed0):**
- **0x09 icons** from the font image's `icon` plane (HD bar2, 21 columns of 24-texel cells):
  - a trunc(28k) − 4 square, 2 px right of the pen, its bottom at baseline + 4k − 1;
  - untinted, at the text alpha, with no shadow;
  - measured at the raw width (18/24) and advanced width·k.
- **Pad prompts:** `09 da..f9` map to pad icons by the device family (table 0x1405c4d40).
- **Colour codes:** 0x07 replaces the RGB and scales the alpha; 0x03 resets.
- **Callback text:** sprites gained `after`, a draw callback's text.

**Message loading:** messages end at a 0 in code position (a 0x07 colour holds zeros); `load_messages_optional` returns `None` for an absent id.

**Font shader correction:** the exe's program table (FUN_14011e1d0: 0x228-byte records at dev+0x7bf8, the PS at +8) gives program 3 (sys glyphs and icons) = ps 026, a plain texture × colour; ps 029 is program 9. The luminance path that T075's addendum added to `ui2d` and the HUD canvas is removed, and the memory note is corrected.

**`test_room`:**
- Loads the item table with its names and descriptions, the screens' messages and the icon plane.
- Room stock and Sora's equipment, as test-room values:
  - stock: Potion ×3, Hi-Potion, Ether ×2;
  - equipment: Kingdom Key; armor, accessory and two item slots, all empty.
- Stock, seen marks and HP/MP carry back to the room when the camp closes.
- The unattended script now reopens the camp and:
  - walks Items to the consumables tab;
  - uses an Ether;
  - uses another, which gets the no-effect box;
  - throws one away;
  - backs out and closes.
- The title binary binds the icon plane too.

## Evidence (private, outside Git)

**`autoplay-65`:** the first run.
- It showed the 60-frame G142 panel tail swallowing presses that a list result never saw. Waits now skip on any press.

**`autoplay-66` / `autoplay-67`:**
- Captures:
  - 34: the Items top with the Kingdom Key and "–––" slots, and the equipment help;
  - 35: the Stock preview;
  - 38: the consumables tab with icons, the LT/RT arrows, "×3" / "×2" counts and the Ether description;
  - 40: the use target with the mode-1 block;
  - 42: the no-effect box with the red INFORMATION header and icons;
  - 44: the Throw-away box with Yes selected;
  - 45: the list after the throw (Hi-Potion, cursor fixed up).
- Log: se #37 at the first use; the camp closes at 3416.
- `autoplay-66` showed dark Yes/OK text. Choice rows keep the font RGB (FUN_1402eb960 clears flag 0x10 only).

**Re-checks:** `title-run-14` and `talk-autoplay-03` re-check the title and talk room with the plain font shader.

## Checks

PASS fmt; PASS clippy; PASS tests (new: item table, stock lists/tabs/flags, message substitution, potion effect, grid input, icon/colour layout); PASS build; PASS smoke; PASS autoplay-67, title-run-14, talk-autoplay-03.

NOT RUN: comparison with installed-game captures; frame capture of the shader program.

## Uncertainty

- **Not built:**
  - Equip (state 2): deciding a member plays the decide sound only.
  - SQUARE → Abilities.
  - The item pictures: the sequence DAT_142a11888 is unresolved.
  - The equipment info rows in state 11 (mode 0 of FUN_1402fd390).
  - Keyboard key-cap prompts: Xbox icons are drawn instead.
- **Unresolved (spec):**
  - the START sound in the stock list;
  - where the "new" bits are filled;
  - FUN_1403c4e50 (inferred: MP not full);
  - the boost caps;
  - the choice list's wrap flag.
- **Test-room values:**
  - the stock and equipment;
  - one member;
  - no Struggle flag.

## Verdict / next

PROGRESS: Items stock, use and throw-away work end to end; menu text has icons and colour codes. Next: Abilities (`camp-06/abilities-spec.md`), then Equip, Customize, Party, Journal.
