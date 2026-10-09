# T073 — Camp (pause) menu top level in the test room; the exe's text layout

Owner Claude, 2026-10-09. Gate P22 (front-end scope). Starts from T072. User request: the pause menu 1:1, with sounds and controls.

## Prerequisites / source

Same selected source and manifest (quarantined, rehashed every launch). Private evidence:
- `research/playable-room-20261008/camp-01/camp-spec.md` (top level, sub-screen map).
- `camp-02/camp-text-spec.md` (text pipeline, help, status block, digits, LightCur).
- Ghidra outputs `ghidra-kh2-a1/camp-02`, `ghidra-kh2-a3/camp-04`, `ghidra-kh2-a2/camp-20`.

Files read in addition to T072: `menu/us/camp.2ld` (kh2_fifth #4538), with its four HD replacements.

## Implementation

- `camp` (rewritten on `sprite2d`), translated from FUN_140306040 / FUN_140306790 / FUN_140367b50 / FUN_140303260 / FUN_1403037d0 / FUN_140303cd0:
  - the layout (scenes 0 → 1, close 2);
  - the party frame and the PC settings prompt;
  - the munny and time labels with Tz::MultiSeq digits: ones digit at the label context + (90, 2), 16 px per digit, 8 px for the colon, a single "0", alpha following the label, the clock rebuilt every frame;
  - the party status block: container (547, 298), member pitch −96, name centred size 15, LV/HP/MP style-group colours, size-12 values right-aligned at +86, HP/MP gauge bars scaled by the ratio;
  - rows: pitch 26, text colour from the row animation, selected row popping out 18 px;
  - the LightCur on row context + (100, 0), with a 30-tick glide that truncates per axis;
  - help text on the layout element + (141, −80), size 18; on a move it only swaps text; it is killed at once on exit;
  - list input: repeat 30/4, wrap only on fresh presses;
  - a press during the open skips the layout to idle;
  - sounds se000 #3 open, #1 move, #2 decide, #4 close (cancel or START);
  - every close group on exit; the camp ends when the layout dies.
- `sprite2d`: the sys-font text layout of FUN_1403aed80 / FUN_1403afd90 / FUN_1403b0280 / FUN_1404070f0:
  - the origin is the top of the first line;
  - glyph cell bottom at line top + lineH + 2;
  - quad 18k×24k truncated to 1/16 px;
  - advance (int)(w·k+0.5); 0x01/0x20 (int)(6k+0.5);
  - line pitch lineH + 5 + UiPadding;
  - alignment by the widest line;
  - font 1's black half-alpha shadow at (+2, +1);
  - the international 0x46 rule.

  Also: parent alpha multiplies keep the child's own RGB multipliers; sprite scale multipliers; `draw_where` for priority bands.
- `ui2d`:
  - font sheets use the font pixel shader (RGB = texel weights 0.299/0.596/0.211 × colour);
  - layout meshes and their camera use their own render layer, so the test room's HUD camera no longer draws them at pixel scale;
  - the overlay camera clears to transparent and is laid over the scene as premultiplied colour.
- `test_room`:
  - a `camp` config entry;
  - START (logical 0x2, field pad mode 1) opens the camp unless talking;
  - while the camp is open the field systems do not run, the HUD is hidden [inferred], the menu reads pad mode 7 and the frame count keeps going;
  - an unattended camp check after the combat plan (1810–1936).

## Evidence (private, outside Git)

`research/playable-room-20261008/autoplay-49` (+ `.jsonl`, config `test-room-arena-05.json`):
- The camp loads with 13 props, 6 scenes and 2 sequences; the 4 HD sheets are 1024².
- Open at 1810 with se000 #3. Moves at 1840/1870/1890 each play #1. Cancel at 1920 plays #4. The camp closes at 1936 and the field resumes.
- Capture 22 shows the MENU header, seven rows with Customize selected and popped out, the orb, the help line "Change Character settings.", MUNNY 0, TIME 0:00, Sora's LV 1, HP 16/20 and MP 50/100 with gauges, and the party frame.
- `autoplay-46/47/48` were rejected intermediate runs (an immediate close, then a camera layer fault); their causes are fixed above.
- `title-run-04` re-checks the title and the dialog with the new text layout.

## Checks

PASS fmt; PASS clippy; PASS `cargo test --workspace --locked` (camp digit rules, glide); PASS build; PASS 180-frame smoke; PASS autoplay-49; PASS title-run-04.

NOT RUN: a comparison against an installed-game capture of the camp.

## Uncertainty

- **Approximation:** the overlay layer composites in Bevy's linear window space. The exe blends 2D straight onto its gamma framebuffer, so semi-transparent pixels over the 3D scene differ slightly. A gamma-exact path would render the 3D scene to an image and compose everything in one gamma-space 2D pass.
- **Not built:**
  - the 3D party models (projection partly known);
  - the "NEW!" badges (predicates undecoded);
  - the sub-screens (decide plays #2 only);
  - Journal;
  - the PC settings overlay;
  - the anchor sprite (G116/117 on DAT_140beecf0).
- **Test-room choices:**
  - entries Items, Abilities, Customize, Party, Status, Jiminy's Journal, Config (the save's menu flags decide these);
  - munny 0, level 1, the clock = the room's frame count.
- **Inferred:** the field HUD is hidden while the camp is open; text alpha is squared during parent fades.

## Verdict / next

PROGRESS: the camp top level opens, navigates and closes as the exe does over the frozen test room. Next: the original controls throughout the test room (pad mode 1 tables in `pad.rs`), the gamma-exact 2D composition, and music once the Vorbis decoder question is answered.
