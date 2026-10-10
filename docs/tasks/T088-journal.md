# T088 — Jiminy's Journal (top pages)

Owner Claude, 2026-10-09. Gate P22. Starts from T085.

## Source

Private evidence: `research/playable-room-20261008/journal-01/journal-spec.md` (Ghidra `ghidra-kh2-a1/jm-01..04`, `ghidra-kh2-a3/camp-04`), and the exe tables read from the installed image: 0x1405ba7e0 (pad mask → prompt code), 0x1405baae8 (top tabs), 0x1405bbc58..0x1405bbca0 (option labels and help), 0x1405baab0..0x1405baac8 (slide, tab and light groups).

## Implementation

**`package_reader::journal_table`:** reads the jiminy.bar `worl`, `stor` and `limi` tables.

**`layout2d`:**
- `.dds` replacement names;
- `load_images` for 2LDs that hold only IMGDs (jm_world, jm_top_<code>).

**`sprite2d`:** `child_alpha`.

**`camp::journal`** covers the whole journal mode.

- **Init** (FUN_1402f3d70) and the frame build (FUN_1402f5410):
  - book layout scenes 0/1/2 (Roxas 3/4/5);
  - page container G153 → 152;
  - help anchor G44 → 45.
  - Jiminy's journal opens on Notes for the current world, Roxas's on his notes, and the journal opens on Select World when there is no world.
- **Frame:**
  - title G53 and L/R tabs G47/G50 (FUN_1402f5780);
  - tab labels "⟨L1⟩ label" / "label ⟨R1⟩" (prompt codes 0xe0/0xe1) from the ring FUN_1402f3b20 / FUN_1402f3bd0;
  - help text G145/G146 (FUN_1402f2940);
  - the slide FUN_1402f5330;
  - intro skip on a press (FUN_1402f4f40);
  - hand G33 and LightCur 35/36/37 at 0x7571 (FUN_1402f2cf0).
- **Select World** (FUN_140326b60 / FUN_1403260f0):
  - 18 rows with their marks;
  - the listed `worl` rows (unlock flag or show-all 0x4837) as cells G173/G172;
  - cell layout 6 × 80 / 98 with the last row centred;
  - world icons on seq 2 from jm_world;
  - the 2-D grid step FUN_1402f8240, translated with its edge, wrap and centring rules;
  - decide → Notes / Collection; cancel / START exit; L1/R1 with the validator.
- **Notes** (FUN_140327d00 / FUN_140328740 / FUN_140327330):
  - P0 G159, P1 G169, Summary G167, Objective G168 with label pills;
  - 9 option rows, pitch 22, filtered by FUN_140327c00;
  - Summary/Objective from the last reached `stor` row, or 0x8adc;
  - title strip (seq 4) and picture (seq 3) from jm_top_<code>;
  - help per option; cancel → Select World; L1/R1.
- **Collection** (FUN_14032bb90 / FUN_14032bf60 / FUN_14032b210):
  - P0 G155, P1 G165;
  - 12 option rows, pitch 24, filtered by FUN_14032b860;
  - pictures at 0x7561.
- **Roxas's notes** (FUN_140328330 / FUN_1403276e0):
  - G163/G166/G170/G171;
  - no title, tabs, hand or LightCur;
  - list mask 0x63, so cancel / START close.
- **Exit** (FUN_1402f2cf0 exit branch + FUN_1402f37c0):
  - book close scene, container G154;
  - frame texts close 55/46/49/52;
  - help and hand killed;
  - once the layout is gone, everything goes.
- **`Curtain`:** pause.2ld seq 0 at 0x7554 — G5 → hold G1, uncover G4.

**Camp:**
- deciding entry 5/6 sets `journal` and the exit flag;
- `kill_all` is the curtain callback FUN_140305480.

**`test_room`:**
- `journal` config (arena-07): jmemo.2ld, jiminy.bar, jm.bar, jm_world.2ld, jm_top_{tt,tt2,jm}.2ld, pause.2ld.
- The chain:
  1. the camp closes under the curtain;
  2. when black, the camp sprites go and the journal opens (se #3);
  3. the journal ends;
  4. the camp is rebuilt (se #3) with the cursor on Journal while the curtain lifts.
- **Test-room values:** world TT (2); no story flags; no limits, reports, pieces, missions, minigames or synthesis.
- The unattended script adds a Journal walk:
  - Notes → down → decide;
  - R1 Collection → R1 Select World → L1 Collection → L1 Notes;
  - cancel → Select World → cancel out;
  - close.

## Evidence (private)

`autoplay-76` (arena-07 via `kh2fm room`, exit 0, 81 captures, none uniform):
- 70: the curtain over the field;
- 71: Notes, Twilight Town:
  - tabs "LB Select World" / "Collection RB";
  - the TT title strip and picture;
  - Summary / Objective "FAKE";
  - rows Characters, Story, Album, Treasures, Maps, Character Links;
  - the hand and LightCur on Characters;
  - help "See the characters involved in this world!";
- 72: the cursor on Story with its help;
- 73: Collection:
  - Jiminy picture;
  - rows Character Files, The Heartless, The Nobodies, Treasures, Maps, Character Links;
- 74: Select World with no cells, help "Which world would you like to look at?";
- 78: the closing fade;
- 79: the camp back with the cursor on Journal.

Log: `journal_open` (kind 1, state 2) at 6216; `journal_close` 6707; sounds 3 / 1 / 2 / 4 as listed in §2.5.

`autoplay-75` (same build before the limits fix) also failed: captures 2–8 (field, ticks 206–456) came out black while another kh2fm (title) process was running. The rerun had none.

## Checks

PASS fmt; PASS clippy; PASS tests (new: journal tables, grid step, tab ring, `.dds` names); PASS build; PASS smoke; PASS autoplay-76.

NOT RUN:
- comparison with installed-game captures;
- Roxas's Journal in the room (no entry 6 in the test-room list);
- the world grid with cells (no story flags).

## Uncertainty

- **Not built:**
  - the category pages (decide plays the decide sound only);
  - NEW / status marks (their predicates FUN_14034a940 … are not traced);
  - the sub-title tabs (FUN_1403240c0), used only by category pages;
  - the image fade-in (G1) for late loads (loads are synchronous here).
- **[U] Select World with no cells:** FUN_1403271a0 reads row −1. The title keeps the previous page's text.
- **[U] START's close sound:** remap[5] gives #2 in the file image and #4 in camp-spec; #4 is used, as in the camp.
- **[I] Roxas's help:** FUN_1403279a0 runs only on a cursor move, which one row never makes, so no help shows.
- **Limits:** FUN_140339500 → FUN_1403e7c30 (03system limit record by command) → FUN_1403e8150 (save+0x3d48 count per record id). This is not modelled; the test-room save has none.

## Verdict / next

PROGRESS: every top-level pause-menu entry now opens its screen. Next: the journal category pages, or the remaining gaps of T079–T085.
