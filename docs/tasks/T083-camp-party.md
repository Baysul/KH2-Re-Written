# T083 — Camp Party screen

Owner Claude, 2026-10-09. Gate P22. Starts from T081/T082.

## Source

Private evidence: `research/playable-room-20261008/camp-08/party-spec.md` (Ghidra `ghidra-kh2-a2/party-01..03`).

## Implementation

**`camp::party` (states 0x14 / 0x16; FUN_140320b30, FUN_14031fbb0, FUN_14031f9a0, FUN_140321420):**
- **Panel:** G312 → 313 (close 314).
- **Cards and name boxes:**
  - one card per member (G315; slot 3 G316) at 118-unit steps;
  - a name box per member (G318 for Sora, G319 for a switchable member), with the name centred (flag 0x10 kept).
- **Card list:** steps LEFT / RIGHT with a skip mask (Sora skipped).
- **Fewer than three members:** prev/next/decide are masked off, there is no hand, LightCur or marker, and the help is 0xbb88. Otherwise the help is 0x846b.
- **Decide** goes to the second pick (0x16, help 0x846c), which swaps the two members (order and names).
- **Marker:** G317 sits on the active hand's card.
- **Cancel** returns to the top level once the panel and tab are gone.

**Member models:** Tz::MenuObj pastes now come from `Camp::model_layer`.
- On the top level: 0x7538, at `model_offset`, with the party frame's alpha.
- In Party: 0x7545 on each name card's context minus the layout centre, with the card alpha × panel alpha. Sora's walk root lands at (75, 281.8).
- The walk clock runs while any model host lives.
- The camp now applies the paste offsets itself; the room only binds the render.

**`MenuList`** gained configurable step keys (`keys`; the dialog choices use RIGHT/LEFT, Party uses LEFT/RIGHT) and a skip mask (+0x20).

**`test_room`:** Party help texts; the unattended script adds a Party visit: reopen, down to Party, RIGHT (nothing to switch, no sound), cancel, close.

## Evidence (private)

`autoplay-71` (arena-06 via `kh2fm room`):
- capture 57: the Party panel with Sora's card, his walking model, the "Sora" name box and "There are no Party Members to switch.";
- the log shows the decide, no sound on RIGHT, cancel #4, and the camp closing at 4556.

## Checks

PASS fmt; PASS clippy; PASS tests; PASS build; PASS smoke; PASS autoplay-71.

NOT RUN: comparison with installed-game captures.

## Uncertainty

- **Not built:**
  - the 0x15 notice (event camp);
  - locked members and world guests;
  - writing the swap back to the world party table at close (no-op in the Sora-only room);
  - the dimmed slot-0 hand during the second pick;
  - the late-load G125 fade.
- **Room:** only Sora's render exists, so other members' cards would show no model.

## Verdict / next

PROGRESS. Next: Customize, then Equip.
