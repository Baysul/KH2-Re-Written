# T072 — New Game dialog, exact screen fade, dk::Sprite text model

Owner Claude, 2026-10-09. Gate P22 (front-end scope). Starts from T071. User request: main menu and pause menu 1:1 with sounds and controls.

## Prerequisites / source

Same selected source and manifest (quarantined, rehashed every launch). Private evidence:
- Spec: `research/playable-room-20261008/newgame-01/newgame-spec.md`.
- Ghidra outputs: `ghidra-kh2-a1/title-02`, `title-20` (FUN_1402b7420 / 7940 / 7e40 / 8470 / 8520 / 8830 / 8910 / 8d60 / 9200 / 92c0, FUN_14016ae30, dk::Fade FUN_140156f10, vtable 0x1405b1998).

Files read in addition to T071: `msg/us/sys.bar` (kh2_first #608), and `msg/us/fontimage.bar` / `fontinfo.bar` (the sys font).

## Implementation

- `fade` (new): dk::Fade translated from FUN_140156d00 / FUN_140156f10.
  - Two opaque in-delay frames, then f32 truncation of `duration × 128/2n`.
  - n = 0 is a special case (1.0 per frame over 2 frames).
  - Black blends normally, white additively.
  - Unit tests reproduce the spec's sequences for n = 0, 1 and 60.
  - The title draws the fade quad last, over x −86..598, y −1..417.
- `sprite2d` (new): the dk::Sprite / SpriteMessage model:
  - priority then creation order, queued groups, close-then-die;
  - parent chains (base = parent context + offset + base);
  - 0x200|0x400 alpha inheritance;
  - die-with-parent rows;
  - FUN_14016ae30 text: flags 4/8/0x10, text offset, size/18, alignment 0/1/2, line pitch (int)(18·scale)+5+UiPadding, control codes skipped;
  - the Tz::LightCur glide.
- `sequence`: group fields +0x18 TextScale, +0x1c UiPadding, +0x20 TextPositionX, and `context_color` (the IsActive animation's colour).
- `newgame` (new): the dialog translated from the routines above.
  - Heading with the dim, description window, page title, 4/2 rows, the confirm page with labels, values and Yes/No.
  - Pointer and orb.
  - List input: repeat 30/4; pages 0/1 wrap on fresh presses; the confirm page uses left/right and clamps.
  - Intro skip on the first press.
  - Cancel graph: page 0 closes, 1 → 0, confirm → 1, No → 0.
  - Sounds se000 #1/#2/#4/#5.
  - Defaults: Standard, vibration ON, Yes.
- `title` binary:
  - The dialog runs inside the title task: the menu frame pauses, the layouts keep animating.
  - Frame order follows the task priorities: layouts/sprites, then the 2D draw, then the title logic, then the fade.
  - A confirm plays exit scene 15 with se000 #21.
- `ui2d`: layout space is stretched to the window (682.67×416 → 16:9; layout units are not square). The earlier aspect-preserving mode left white side strips.
- `camp` (new, not yet wired): the camp top-level state machine, sprites, LightCur glide, help, munny/time digits. Its text placement waits on the camp text research.

## Evidence (private, outside Git)

`research/playable-room-20261008/title-run-02` and `title-run-03` (+ `.jsonl`).

Script: down, up, New Game (780), Proud (840), next (880), vibration ON (940), cancel on confirm (1000), confirm (1060), Yes (1120).

Log:
- se000 #1, #1, #2, #1, #2, #2, #4, #2, #2.
- `new_game_dialog` confirmed at 1138 with mode 2 (Proud) and vibration ON.
- se000 #21 and exit scene 15 at 1138; hand-off at 1390.

Captures in title-run-03:
- 07: the page 0 window, Standard selected, with pointer, orb and the 3-line description.
- 12: the dim and heading between pages.
- 13: the confirm page fading out (Game Mode: Proud Mode, Vibration: ON, "Begin with these settings?", Yes/No).

## Checks

PASS fmt; PASS clippy; PASS `cargo test --workspace --locked` (new: fade 1, sprite2d 1, newgame 2, camp 1); PASS build; PASS 180-frame smoke; PASS title-run-03.

NOT RUN: a comparison against an installed-game capture.

## Uncertainty

- **Inferred:**
  - Per-line centring of multi-line text.
  - The text colour source (the IsActive animation's colour) when both an IsActive and other animations run.
  - Glyph RGB at 0x80 = 1.0 (the same convention as the vertex colours).
- **Unresolved (from the spec):**
  - The trailing `02 07 f0 f0` on mode descriptions: the code is skipped here.
  - The Critical bonus table.
  - The saved vibration/difficulty apply: the test build only logs the choice.

## Verdict / next

PROGRESS: the New Game dialog and the global fade follow the exe. Next: the camp (pause) menu wired into the test room on the shared sprite model, the original controls in the test room, and the title music once the Vorbis decoder question is answered.
