# T077 — Camp Status and Config screens, glove pointer, tabs

Owner Claude, 2026-10-09. Gate P22. Starts from T073/T076. User request: pause menu 1:1.

## Prerequisites / source

Private evidence: `research/playable-room-20261008/camp-04/status-config-spec.md` (Ghidra `ghidra-kh2-a4/camp-31`), plus camp-01/02.

## Implementation

**`camp` rewritten as a state machine** (state, substate 0/1/2, pending, exit), translated from FUN_140303260 / FUN_140303cd0:
- top level (0), Status character list (0x17), stat browser (0x18), Config (0x24);
- the list object with the exe's input codes: up/down wrap on fresh presses, L1/R1 page keys, left/right −12/−13, L2/R2 −10/−11, and its enable masks;
- FUN_1402f6270 sounds: #1 move and left/right, #2 decide and L2/R2, #4 cancel and START, #5 invalid.

**Top level → sub-screen** (FUN_1403037d0):
- In the same frame: the tab label is set, rows play 135, help G117, party frame 48, status block 103, prompt 353, substate 2.
- The driver kills the glove and closes the LightCur (29).
- The commit waits until the rows and status block die; a press during the wait kills them at once.
- The return rebuilds the top level with the cursor on the remembered entry, and no sound.

**Shared pieces:**
- the white glove pointer (G25, prio 0x754a), parented to the cursor row, top level included;
- title tabs (G75 → 76 / 77 at 120·i) with their own orbs;
- help that swaps text without re-fading and restarts G116 after a fade-out;
- a per-camp `Memory`: entry, Status bit, Config row.

**Status 0x17:**
- character rows (character and form/Summon groups);
- panel G140 → 141 → 142;
- 20 label/value pairs compacted per column at (170·col, 24·rank), with style groups 137/143/150/139/144 and yellow right-aligned values at +160;
- per-character help.

**Status 0x18:**
- tab 1 with the name;
- panel G320 → 321 → 322;
- two columns of rows (style groups 323–327) at (232·col, 24·rank) with values at +180;
- left/right switch columns (cursor clamped), L1/R1 page by 10, L2/R2 switch character (invalid #5 with one character);
- the "Switch" banner G237 → 238 → 239;
- per-stat help.

**Config 0x24:**
- panel G233 → 234 → 235;
- 8 labels (G279, size 13, centred);
- 1/2/3-choice rows (groups 261/260, 255/254, 258/257; pitches from UiPadding) with the selected look;
- the cursor frame (253/256/259) on the selected choice, and the LightCur on the choice;
- left/right change the value with wrap; decide is masked off; row 7 (Game Mode) is read-only;
- values are read from and written back to save+0x41a4 on cancel or START (`config_values` / `config_flags`, FUN_140365460 bit rules).

**`sprite2d`:** sprites keep their last child context and colour while no IsActive animation runs (FUN_140192be0). Without this, children of a panel whose close group sat in its invisible tail (G142 runs to loop end 60) were drawn at the origin.

**`encode_number`:** '%' → 0x4a.

**`test_room`:**
- the camp's Status data is Sora at level 1: LVUP row EXP for Next LV, PLRP AP/Str/Mag/Def as in the combat setup, the room's HP/MP, resistances 0% [test-room values];
- sys.bar labels and help;
- Config texts;
- config flags start at the new-game 0x40b with Standard mode, and persist across openings;
- the unattended camp walk-through now covers Status, the browser, Config, back out and close.

## Evidence (private, outside Git)

`research/playable-room-20261008/autoplay-61` and `autoplay-62` (arena-06):
- The camp opens at 1810 and closes at 2496.
- The log shows every list sound in order.
- Captures:
  - 21: the Status list with Sora's 14 stats in two columns and "Sora's Status." help;
  - 25: the stat browser with tab "Status" › "Sora", the Switch banner, the glove on Magic, the orb and its help;
  - 30: Config with Field Camera Manual, Summon Effects Full under the cursor frame, and its help;
  - 22: the clean 0x17 close.
- `title-run-13` re-checks the New Game dialog on the changed sprite model.

`autoplay-60` and `camp-debug-01` were diagnostic runs:
- the first found the lingering-children bug;
- the second, with temporary logging that has since been removed, traced it.

## Checks

PASS fmt; PASS clippy; PASS tests (new: Config bit round trip, list paging/codes); PASS build; PASS smoke; PASS autoplay-62, title-run-13.

NOT RUN: comparison with installed-game captures.

## Uncertainty

- **Test-room values:**
  - The status numbers.
  - The list holds only Sora (no forms, Summon or party).
  - The 0x18 banner's two inline button icons (code 0x09 glyphs) are not drawn.
- **Unresolved (spec):**
  - PC keys for L1/R1/L2/R2 in menus;
  - FUN_140101f60;
  - the consumers of the Config bits;
  - the battle-stat formulas.
- **Inferred:** text alpha is squared under fading parents. The rows' and panel's exact first/last frames need a capture.

## Verdict / next

PROGRESS: Status and Config work end to end from the top level. Next: the party models in the camp frame (`camp-03/menuobj-spec.md`), then Items/Abilities/Customize/Party.
