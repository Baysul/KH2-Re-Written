# T093 — Journal Characters / Links / Album, tutorial notices, keyboard key caps

Owner Claude, 2026-10-10. Gate P22. Starts from T088 (the Journal top pages) and T090 (item pictures).

## Source

Private evidence under `research/playable-room-20261008/`: `journal-01..02` (state machine, `char` / `diag` / `albu` tables), `camp-05/items-spec.md` and `camp-09/equip-spec.md` (notice triggers), and the decompile sets for FUN_14032d350 (the `diag` layout), FUN_14032db20 / FUN_14032a670 (face and photo loaders), FUN_140367300 / FUN_1403662c0 / FUN_140366850 / FUN_140367150 (tutorial notices), FUN_1403adcc0 (font encoding).

## What was missing

- The Journal's Characters, Links and Album pages (`Notes of the world` → `Characters`, `Album`) were stubs that bounced back to the Notes page, so no character faces, no album photos.
- The camp's first-time tutorial notices (the `menu/us/<name>_<v>.bar` pictures with their callouts) never showed.
- Keyboard key caps in help lines and callouts (`09 da..f9` codes) drew Xbox icons only.
- HD replacement images were taken in the HED's lexicographic order, so a sheet with ten or more images was misassigned (Donald's face showed Riku).

## Findings

- **Faces:** `menu/jmface_<code>.2ld` per journal world (`ex` for Links, world 0x10); the HD replacements are named `-N.dds` and stored in lexicographic order, so `-10` sorts before `-2`. They must be re-sorted by number. `layout2d::load_images` now does.
- **Photos:** `menu/us/jm_photo/<worldcode><nn>.bin`, a bare IMGD; `<nn>` is the `albu` record's +1..+2.
- **Characters layout:** `diag` rows (cond, off, world, kind, entries of 8 B) give each face position; FUN_14032d350 places them by base layout (kind 0), flag extras (kind 1), item extras (kind 2). The free hand moves 3 px/frame, or `|stick|*6 + 0.5` on the analog stick, inside x 60..490 / y 80..370 from (275, 225); hit half-size 32; face frame group 10+ft (links 16+ft), NEW marks 24/25 (27/28), highlight 22/26.
- **Album rows:** 8 rows at pitch 26 from (148, 100); NEW marks 92/93; the photo draws at the panel context (112, 326) + (160, −113); R2 / L2 (−10 / −11) step the photo.
- **Notices:** variant r / o / s from story flags 0x8db / 0x8d0 / 0x8d1 and the row mask; `THelpText.bin` (0x10 header, count u16 at +6, 0x180-byte records, callouts at +0x90 + 0x30k + 8L: i32 message, i16 x, i16 y; US L = 1). Triggered on SELECT of Items (stock), Equip slots, Abilities and Customize entries, once per kind (`Memory.notices_seen`).
- **Key caps:** `Font.pad == 0` marks the keyboard font; cell → key table from the default bindings (0xe2 → `C`, 0xe3 → `E`, 0xdf → `F`, 0xe0 → Shift, 0xe1 → `R` …).
- **Locked world title** uses code 0x49 (`?`), not 0x3f.

## Implementation

- **`package_reader::journal_table`:** `Char`, `Diag`, `DiagEntry`, `Album` and `read_chars` / `read_diag` / `read_albums`; test `reads_chars_diag_and_albums` on synthetic bytes.
- **`package_reader::bin::archive_probe`:** keeps plain (non-BAR) files so `THelpText.bin` can be probed.
- **`layout2d`:** `IMGD` / `.dds` names, `load_images_named`, `load_imgd` (bare IMGD or BAR's first IMGD), HD sorted by number.
- **`sprite2d`:** `keyboard_prompt`, key-cap quads for `Font.pad == 0`, `child_alpha`.
- **`camp/notice.rs`:** the notice state (`State::Notice`), page container, callout placement, `NoticeCache`; fades with the picture's alpha.
- **`camp/journal/{category,characters,album}.rs`:** category frame and pill, Characters / Links hand walk with face groups and NEW marks, Album list and photo; `JournalCache` asks the host for faces and photos; seen rows are carried between openings.
- **`test_room`:** `JournalConfig.flags` (`journal.flags` in the room config; 0x4837 shows everything), loaders for notices, faces and photos, camp present mappings, an unattended walk through Characters, Album, photo, R2, back out and Select World.

## Evidence (private)

- `autoplay-85`: Characters page with the free hand walked left and down onto Donald, faces correct (Sora, Donald, Goofy …), frame highlight and NEW marks.
- `autoplay-86` (arena-10 via `kh2fm room`, exit 0, captures at ticks 6330..7330): the Album list (rows A Message / Awakening / The King's Teacher with NEW marks), the photo page (A Message, Awakening with its picture and caption, the `C` `E` key caps in the help line), the camp returning as the curtain lifts.
- The top menu capture (autoplay-86, tick 2420) shows the `Esc` key cap beside the gear icon; the Items help line shows the `C` / `E` key caps.
- Tutorial notices were checked by screenshot in earlier, uncommitted runs (callouts and the `F` key cap at idle); the committed walk does not press SELECT on Equip / Abilities / Customize, so no notice capture is part of autoplay-86/87.

## Checks

PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked`; PASS `cargo build --workspace --locked`; PASS smoke `cargo run -p windows_shell --locked -- --frames 180`; PASS autoplay-85 / 86 (exit 0, no rejected or panic lines).

NOT RUN: a committed walk step that triggers a tutorial notice (SELECT on an Equip slot, Abilities or Customize entry); pixel comparison against installed-game captures; Links page with a save that has Links unlocked; the notice's per-save "seen" persistence (held in the room's memory only).

## Uncertainty

- **[I]** Fade timings of the Album list and photo come from the page container groups; no installed-game capture was compared.
- **[I]** The analog stick curve for the Characters hand is `|s|*6 + 0.5` px/frame from the decompile; the room's walk used the digital pad only.
- The notices' once-only flags are not written to a save.

## Addendum: Character Files family (same task)

Source: `journal-03/files-treasures-spec.md` §2.2, §3.1–§3.4.

- **Built:** world menu (state 0xe), the lists (0xf Character Files, 0x12 The Heartless, 0x15 The Nobodies) with a 10-row scrolling window, NEW / viewed marks, "There's nothing yet!", the second-level tab with the world name, and the detail page (0x10 / 0x13 / 0x16): portrait from `menu/jmface_<code>.2ld` (codes `heart`, `no`, `ex` added) in its frame, name, origin note, description in 7-line windows with the scroll arrow, R2 / L2 to the next / previous seen entry, back to the list or to the Characters page it came from. The Characters page's decide now opens the detail.
- **Evidence (private):** `autoplay-91` (arena-10, exit 0): the Character Files world menu, the Other list (Sora, Donald Duck, Goofy, Kairi …) with its "Other" tab, the Sora detail with portrait, the Donald Duck detail after R2 (origin note "The Wise Little Hen (1934)"). `autoplay-90`: The Heartless shows "There's nothing yet!" (no kill counters in the room, as the spec gives for a fresh save).
- **Also fixed:** the face cache was not reused when the journal reopened, so portraits were missing on the second opening.
- **Not built:** the 3D model and the full-model pages (0x11 / 0x14 / 0x17, TRIANGLE), the "Slain" count and reaction rows (kill counters and `rcct` are not in the room's data), the list scroll bar G95.

## Addendum 2: Treasures and Maps grid

Source: `journal-03/files-treasures-spec.md` §4.
- **Built:** states 7 / 0x1a (Treasures) and 9 / 0x22 (Maps): the frame with caption bar, 9 world-label rows with "There's nothing yet!", 72 cells (chest / map / "?" squares), cursor frame with hand and light, the "Total n/m" counter under show-all, caption ("?????" until opened, item with icon after), NEW / viewed marks, and the movement rules (§4.6). Data: 03system `trsr` (new `journal_table::read_treasures`, `journal_world`), item names and map type from `item` and the system messages.
- **Evidence (private):** `autoplay-94` (arena-11 = arena-10 plus show-all flag 0x4837, opened bits 1, 2, 3, 20; exit 0): Treasures grid with 301 entries, `Total 2/301`, cursor frame on the first cell and after moving down and right.
- **Not verified / not built:** opened cells (chest icon) and the caption of an opened chest are not in a capture; world labels show "?????" because the room's save has no world unlock flags; the scroll thumb G69's bar object is not drawn; Pieces (8 / 0x21) and Ansem Reports not built; opened chests and owned maps come from `journal.opened_bits` / `owned_items` of the room config.

## Addendum 3: Ansem Reports

Source: `journal-03/files-treasures-spec.md` §5.
- **Built:** the list (state 0x18: 13 `anse` rows, owned ones by title, others "?????", NEW / viewed marks, decide invalid on unowned) and the reader (0x19: title pill, 9-line scrolling text, up / down, L1 / R1 page, R2 / L2 to the next / previous owned report, back to the list). New `journal_table::read_reports`.
- **Evidence (private):** `autoplay-95` (arena-12 = arena-11 plus owned items 0xe2, 0xe3; exit 0): the list with two owned reports and eleven "?????" rows, report 1 and (after R2) report 2 in the reader.
- **Not built:** the scroll bar thumb; whether the option is hidden without reports relies on the existing unlock check.

## Not built yet

Character File 3D models / full-model pages, Story, Treasures / Maps / Pieces (7, 8, 9, 0x1a, 0x22), Ansem Reports, Missions, Minigames, Limits, Synthesis, Puzzles; form sub-weapons on Equip (state 6); NEW! badges on the main menu rows; party members' models; Load / Save (`save.2ld`); item shop (`itemshop.2ld`); mission and minigame 2LDs.

## Next task

Treasures / Maps / Pieces grid and Ansem Reports from the same spec (§4, §5), then Puzzles and the remaining Journal pages.
