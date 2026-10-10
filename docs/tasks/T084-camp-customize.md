# T084 — Camp Customize screens (Sora)

Owner Claude, 2026-10-09. Gate P22. Starts from T083.

## Source

Private evidence: `research/playable-room-20261008/camp-07/customize-spec.md` (Ghidra `ghidra-kh2-a1/cust-01..02`).

## Implementation

**`package_reader::command_table`:** 03system `cmd`. It has 0x30-byte records with the name message at +8. The item record now exposes +0x10, the command a consumable runs.

**`camp::customize`:**

- **Character list (0x19; FUN_14035d2a0 / FUN_14035aa80 / FUN_14035e670):**
  - character rows;
  - an 18-slot preview with category bars {149, 144, 143};
  - shortcut slots read "[button] name", with button glyphs 0xdb / 0xdf / 0xde / 0xda;
  - help 0xbb89 / 0xbb8a.
- **Sora's settings list (0x1a; FUN_14035ccb0 / FUN_14035d6a0 / FUN_14035a4e0):**
  - the work area (FUN_14035b440): 4 shortcuts (save+0x36f8 command ids mapped back to items), one MP Charge row per owned spell (save+0x3744 bits), one Auto-Reload row per distinct item in his item slots; a shortcut copies the state of its item's later row;
  - the panel G227 → 228;
  - 9 rows with category bars and headings (each shortcut row has its own glyph heading);
  - the scrollbar on the panel group, page 9;
  - the Switch banner;
  - the cursor frame G249 and the △ bubble on non-shortcut rows;
  - the validator: L2/R2 need another member, Cure's MP Charge is fixed (se #5);
  - △ toggles MP Charge / Auto-Reload;
  - decide opens a popup; cancel goes back to 0x19;
  - cursor memory with the §9 fix-up.
- **Popups (0x1d–0x1f; FUN_14035c750 / FUN_14035dde0 / FUN_14035a180):**
  - per-category panel, header, rows and frame; the scrollbar; ≤ 8 rows; the cursor starts on the current value (FUN_14035c2a0);
  - the shortcut list: "–––", then owned spells, then consumables in table order, each with its command name;
  - MP Charge / Auto-Reload choices and help;
  - applying writes the shortcut command id, the charge bit or the slots' reload items.
- **Warning box (0x1c; FUN_14035c710 / FUN_14035a080):** turning Auto-Reload off when a used-up slot would empty asks first. A "Yes" from a popup applies on the popup's exit.

**Camp wiring:**
- top-level entry 2;
- the G22 hand in the popups;
- cursor memory slots;
- `Data.customize`;
- `Ui::dialog_box`.

**`test_room`:**
- the command table and the consumables' command names;
- Customize texts;
- shortcuts and charge bits persist across openings (empty shortcuts at start [test-room value]);
- the unattended script adds a Customize walk: Sora → shortcut ○ → Potion → back out.

## Evidence (private)

`autoplay-72` (arena-06 via `kh2fm room`):
- 58: the settings list, four shortcut rows with B/Y/X/A headings;
- 59: the shortcut popup with "–––", Potion and Hi-Potion, and the "Remove Shortcut settings." help;
- 60: slot ○ set to Potion, with its description;
- 61: the 0x19 preview showing "[B] Potion" and the other slots empty.

The log shows the decide/cancel sounds and the camp closing at 5206.

## Checks

PASS fmt; PASS clippy; PASS tests (new: command table, popup start cursor); PASS build; PASS smoke; PASS autoplay-72.

NOT RUN: comparison with installed-game captures; the MP Charge and Auto-Reload paths (the room has no magic and no items in Sora's slots).

## Uncertainty

- **Not built:**
  - party members' settings (0x1b, battle style 0x20, ability frequency 0x21, item frequency);
  - NEW! marks;
  - the SELECT notice;
  - the item-slot re-sort FUN_140357f40;
  - the hand priority 0x7545.
- **Unresolved (spec):**
  - spells' command names come from a runtime table [U], so the item name is used instead;
  - save+0x36e8 candidates [U].
- **Test-room values:** the shortcuts.

## Verdict / next

PROGRESS. Next: Equip (`camp-09/equip-spec.md`).
