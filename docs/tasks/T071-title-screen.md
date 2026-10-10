# T071 — Title screen: layout player, GPU 2D drawing, title task and pad mapping

Owner Claude, 2026-10-09. Gate P22 (front-end scope). Starts from T070. User request: rebuild the main menu (title) and pause menu 1:1, with all sounds, music and controller controls.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence:
- Title spec: `research/playable-room-20261008/title-01/title-spec.md` (files, LAYD, scenes, title task, input, sounds, BGM).
- Controls and music spec: `bgm-input-01/bgm-input-spec.md` (pad layers, keyboard defaults, mode tables, music files).
- Ghidra outputs: `ghidra-kh2-a1/title-01..16` (layout player FUN_140194550/780/8c0/c90/ff0, FUN_140194a40, FUN_140192c60/dc0/e80, Tz::Layout update 0x1402ec5a0, title FUN_1402b35a0/4480/3c10/3d20).
- Base pad table 0x1405c3420 read from the exe image (12 entries, listed in `pad.rs`).

Files read: `menu/us/title.2ld` (kh2_first #574) with its 12 HD replacements, and `se/se000.win32.scd` (#867).

## Implementation

- `package_reader::layout` (new):
  - LAYD reader (props 0x18 B, scenes 0x14 B with the +8 last-frame cap, SEQD offset table) and IMGZ reader;
  - `ScenePlayer`, translated from FUN_140194550 / 780 / c90 / 8c0: end = latest `show_at + group length − 1` (FUN_140192c60), looping when any prop group has play count 0, props start at `show_at`, a scene finishes when no prop is busy;
  - `LayoutTask` (Tz::Layout update): pending scene, queued next scene, `just_switched` for one frame, death.
- `windows_shell::layout2d` (new): loads a 2LD, maps `<LANG>_<name>_2ld<N>.png` replacements by N (integer upscale checked), draws a task's shown props.
- `windows_shell::ui2d` (new): GPU drawing in the original 2D space (682.67×416 at 16:9, no X origin). A `Material2d` reproduces SEQD blends 0/1/2 (alpha, additive, reverse-subtract) through pipeline specialization. Colours are texture × vertex byte/128, clamped, composited in gamma space (`CompositingSpace::Srgb`, sheets uploaded as non-sRGB).
- `windows_shell::title` (new): the title task translated from FUN_1402b4590 / FUN_1402b35a0 / FUN_1402b3c10 / FUN_1402b4480 / FUN_1402b3d20. It covers:
  - start scene 17 (or boot splash 8), then intro 12/13, then idle 14;
  - decide/START skips the intro only;
  - the cursor layout from idle, with cursor scenes {0,1,18} / {2,3,4,19};
  - wrap without repeat;
  - se000 #1 / #2 / #21;
  - the Back fade, the Load hand-off, the 5100-frame idle demo;
  - exit scenes 15/16 and the BGM fade request.
- `windows_shell::pad` (new): the exe's input layers.
  - XInput and keyboard to PS2 PADDATA, with the 0.2 Axa dead zone and the circle→square stretch.
  - The raw word: decide/back bits 24/25, the L2 right-stick role bits 26/27, stick direction bits at 60 byte units.
  - Logical bits through the base table plus the mode-1 (field) and mode-7 (title) tables, all bits of an entry required.
  - The 30/4 repeat.
- `title` binary (new): fade-in from black over 0x3c before the layouts start, the title task at 60 Hz, the default keyboard/mouse bindings and pad, the device-glyph sheet swap (replacements 8–11), sounds, JSONL log and an unattended plan. The `title` binary with no arguments reads `.local/test-room/launch-title.json`.

## Evidence (private, outside Git)

`research/playable-room-20261008/title-run-01` (+ `.jsonl`):
- The load reports 105 props, 20 scenes, 8 sequences, 9 IMGDs and 12 replacements (images 0–8 match the spec's sizes; 9 and 11 are 512², 10 is 1024²).
- Scene 17 runs ticks 122→183 (61 frames), as the spec gives. The intro 12 runs 183→664 (481 frames). Then idle 14 starts, with the cursor layout on scene 0.
- Down, down, up move the rows 0→1→18→1 with se000 #1 each time. Decide on LOAD gives the file-menu hand-off and no sound, as in FUN_1402b35a0.
- Captures 00–12 show:
  - the black fade;
  - the sketch pans;
  - the logo assembly and the row flip-in;
  - the idle title with the Esc GAME SETTINGS glyph;
  - the selected-row art (orb, black label, red bar).

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (new: layout 3, title 4, pad 4, layout2d 1); PASS build; PASS 180-frame smoke; PASS title-run-01.

NOT RUN: a comparison against an installed-game capture of the title.

## Uncertainty

- **Inferred:**
  - The screen fade's per-frame update: alpha 128/(2n) per frame. Only its setup FUN_140156d00 was read.
  - The subtractive blend's factors (destination − source × alpha, as in the gauge canvas).
  - The gamepad name → glyph sheet choice. The exe asks Steam Input for the type.
  - Stick bytes from Bevy's gamepad values.
  - The 16:9 mapping.
- **Not built:**
  - The title music: `Title.win32.scd` is 4-channel Vorbis; the decoder decision is open with the user.
  - The New Game dialog: decide confirms and back cancels in its place.
  - The file menu, the Theater, the Game Settings overlay (Esc quits instead), and the idle demo target.
  - The first-boot controller and storage dialogs.
  - The save scan, so Theater stays locked.
- **Timing:** sound/BGM housekeeping calls at title start (0x140138d20, 0x14013a150, 0x140137c00) have unknown meaning.

## Verdict / next

PROGRESS: the title plays 1:1 from the exe's layout player and title task on the original art, with its sounds and controls. Next: the New Game dialog (research running), the title music once the decoder question is settled, then the camp (pause) menu from `camp-01/camp-spec.md`.
