# T081 — Camp Abilities screens

Owner Claude, 2026-10-09. Gate P22. Starts from T079/T080. User request: pause menu 1:1.

## Prerequisites / source

Private evidence:
- `research/playable-room-20261008/camp-06/abilities-spec.md` (Ghidra `ghidra-kh2-a2/abil-01..04`);
- `guard-01/guard-spec.md` §6 (the Guard ability gate).

## Implementation

**`package_reader::item_table::read_stats`:** the equipment stat table after the item records. It has 0x10-byte records, binary-searched by FUN_14041e8f0, with the ability at +2. The Kingdom Key's stat row gives Damage Control.

**`camp::abilities`:**

- **Work area:**
  - **Built from (FUN_140360920 / 61200):** the member's 80 ability slots (bit 15 = equipped) and the weapon's ability.
  - **Categories:** kind 0 → Support, 1/2 → Action, 3 → Growth, weapon → Weapons. The AP cost is record +7.
  - **Sort (FUN_1403632f0):** exchange sort by category, then item-table order inside a category.
  - **AP used:** the sum of equipped costs.
- **0xf, character list (FUN_140362ca0 / 14035fd30 / 1403642a0):**
  - rows at 0x7541;
  - the preview panel (G140 → 141) with up to 20 equipped abilities, in bar groups {137, 139, 143, 150} with the "on" name icon 0x0d; the panel is not drawn when there are none (flag 2, `Sprite.hidden`);
  - help 0xbb8b;
  - the "□ Items" banner 0xb373; SQUARE goes to Items.
- **0x10, ability list (FUN_1403623b0 / 140363db0 / 140363510 / 14035f810):**
  - the panel G230 → 231 and up to 9 rows with bars by category {248, 252, 262, 265};
  - names grey when they cannot fit the AP left; AP cost cells (G251);
  - category tabs at each category's first visible row (G276/278/280/283, size 13, centred);
  - the scrollbar on the panel's group;
  - the 1×9 scrolling list with L1/R1 page 9;
  - the AP gauge G272 → 273 with "used/max" and the current (G270) and preview (G269 gain / G271 loss) bars;
  - the cursor frame G249 and the △ bubble G24 on the hand when the ability can be toggled;
  - the banner "LT RT Switch □ Items";
  - TRIANGLE toggles (validator: equipped, or the cost fits);
  - decide opens the popup (valid only when it costs AP and fits);
  - L2/R2 are invalid with one character; "There are no Abilities." (G236).
- **0x11–0x13, Equip popup (FUN_140361a00 / 14035f610):**
  - per-category panel, header and rows;
  - the ability name and AP cost (G152);
  - "Don´t Equip" / "Equip", with the cursor on the current state;
  - the scrollbar (2 of 2) and tab 2 with the category;
  - the gauge previews the highlighted choice; deciding writes the slot back.

**Shared:**
- a third title tab;
- the SQUARE / TRIANGLE / SELECT list codes (−5 / −6 / −7; trig 0x800 / 0x1000 / 0x2000);
- the generic banner (FUN_1403035b0);
- Items now offers SQUARE → Abilities with its 0xb619 banner (FUN_140362200);
- the glove uses G22 in the popup.

**`test_room`:**
- Sora's slots hold Guard (0x52) equipped [test-room value, per the guard-01 recipe]; the weapon ability comes from the Kingdom Key's stat row.
- Status shows AP used/max.
- Unequipping Guard in the camp disables guarding in the room, because Guard's ability bit gates PTYA 0x0b (FUN_140402750, guard-01 §6).
- The unattended script adds an Abilities walk: unequip Guard with TRIANGLE, re-equip it in the popup, go to Items and back with SQUARE, close.

## Evidence (private, outside Git)

**`autoplay-69` (arena-06 via `kh2fm room`):**
- Captures:
  - 47: the character list with the Guard / Damage Control preview, its help and the "□ Items" banner;
  - 49: the list with the Action/Weapons tabs, AP 2 cell, scrollbar, AP 2/2 gauge and the Damage Control description;
  - 50: Guard unequipped by △, with the gauge at 0/2 and the bubble;
  - 52: the popup with Guard AP 2, Equip selected and its help;
  - 53: Guard re-equipped at 2/2;
  - 54 / 55: Items ↔ Abilities by SQUARE.
- Log: the camp closes at 4256.

## Checks

PASS fmt; PASS clippy; PASS tests (new: the stat table, entry order and AP used); PASS build; PASS smoke; PASS autoplay-69.

NOT RUN: comparison with installed-game captures.

## Uncertainty

- **Not built:**
  - Drive forms and Lion rows (form records), party members and their frequency cycle (the callback colour is [U]);
  - NEW! badges (the producer of the "new" bits is [U]);
  - the SELECT tutorial notice;
  - SQUARE → Equip in the list.
- **Test-room values:** Sora's slots and AP (LVUP + PLRP).
- **Other open points:**
  - The `<73>` byte in 0xb627/0xb629 is drawn as a glyph.
  - The scrollbar thumb uses the stock list's formula, not the end −1 edge case.
  - The glove pivot rule is not applied.

## Verdict / next

PROGRESS: Abilities works end to end for Sora. Next: Customize, Party and Journal, then Equip.
