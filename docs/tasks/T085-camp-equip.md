# T085 — Camp Equip screens

Owner Claude, 2026-10-09. Gate P22. Starts from T084.

## Source

Private evidence: `research/playable-room-20261008/camp-09/equip-spec.md` (Ghidra `ghidra-kh2-a4/equip-01..03`).

## Implementation

**`camp::equip`:**

- **State 2, slot list (FUN_140357710 / FUN_140358c50 / FUN_140355ae0):**
  - weapon, armor, accessory and item slots from the member's record;
  - the panel G227 → 228 with 9 rows, section bars {265, 252, 248, 262} and section labels {283, 278, 276, 280} on each section's first visible entry;
  - empty slots show their category glyph and "–––"; an emptied auto-reload slot is red (G122);
  - the scrollbar, the holder G287 → 288, the cursor frame G249;
  - the △ bubble on the Items section;
  - the banner "Switch □ Abilities".
  - Inputs:
    - △ toggles auto-reload (FUN_140354dc0); the 0x846a box comes first when it would empty a slot;
    - □ goes to the member's Abilities list (FUN_140362200(0x10));
    - L2/R2 need another member;
    - cancel returns to Items after the tidy of FUN_140357e30 (empty armor/accessories last, item slots grouped by item in table order).
- **Info panel, mode 1 (FUN_1402fd390):** a label per stat the item has (FUN_1402fd100 rules), the item value and the member's total in yellow, and the weapon's ability row.
- **States 5, 7–9, pick lists (FUN_140356b00 / FUN_1403581a0 / FUN_140355770):**
  - per-section panels, headers, rows and holders;
  - the current item in the header, plus the weapon's ability (G124);
  - candidates of the slot's types in stock with "×N" counts;
  - weapons show ability boxes and "None" when empty; armor, accessories and items start with the unequip row;
  - the cursor starts on the equipped item.
  - Validator: the item going back must fit in stock (≤ 98); weapons need a candidate.
  - Accessory AP check (FUN_140359200) with the 0xc22e OK box.
  - Equipping (FUN_140354ea0): old → stock, new ← stock; an item slot's auto-reload clears.
  - Leaving: auto-reload follows the slot items, and emptied auto-reload slots refill (FUN_140355410 / FUN_1403a1610).
- **Comparison, modes 2 / 3 (FUN_1402fd410):** current value, candidate value and the resulting total per stat, cyan for better, red for worse.

**Items:** deciding a member row opens Equip (the Equip memory resets when the member changes).

**Abilities:** `enter_ability_list_for`; L2/R2 validity counts the party.

**`package_reader::item_table::read_stats`:** resist bytes (+9..+0xc).

**`test_room`:**
- Equip texts and stat records.
- Sora's totals: the Status numbers, resists none.
- Sora's equipment persists across openings: Kingdom Key; one armor, one accessory and two item slots [test-room values].
- The unattended script adds an Equip walk: Items → Sora → item slot 1 → Potion → △ auto-reload on → back out.

## Evidence (private)

`autoplay-74` (arena-06 via `kh2fm room`):
- 62: the slot list with the Kingdom Key and empty slots, and the info panel: Strength 3 [5], Magic 1 [7], Ability Damage Control, with the keychain description;
- 65: the items pick list: unequip "–––", Potion ×3, Hi-Potion;
- 67: Potion in item slot 1 with the AT (auto-reload) glyph after △, and the △ bubble;
- 68: the Items preview showing the Potion with the AT glyph.

The log shows the move/decide/cancel sounds and the camp closing at 6046.

## Checks

PASS fmt; PASS clippy; PASS tests (new: info row masks and values); PASS build; PASS smoke; PASS autoplay-74.

NOT RUN: comparison with installed-game captures; the weapon/armor/accessory comparison paths (no such items in the room's stock).

## Uncertainty

- **Not built:**
  - form sub-weapons (state 6);
  - item pictures;
  - NEW! badges;
  - the SELECT notice;
  - the status recompute after equipping (FUN_1403c2100 internals [U]): totals stay as built;
  - the weapon-change list;
  - the save+0x36e8 candidates;
  - the hand priority 0x7545.
- **Resists in mode 2:** the resist totals use status − cur + cand rather than the slot-product formula of FUN_1402fcde0.

## Verdict / next

PROGRESS: all top-level pause-menu entries except the Journal now open their screens. Next: the user's title-sequence request (logos, legal screen, splash).
