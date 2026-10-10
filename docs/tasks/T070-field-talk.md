# T070 — Field talk: message window, text and Axel as a test NPC

Owner Claude, 2026-10-09. Gate P22 (test-room scope). Starts from T069. User request: rebuild the dialogue system 1:1 with a test NPC Axel, with animations.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence:
- Spec: `research/playable-room-20261008/dialogue-01/dialogue-spec.md`.
- Ghidra outputs: `dialogue-01/g/dlg-01..39`.
- Window SEQD and message dumps: `dialogue-01/*.txt`.

## Findings

1. **Axel.** Axel's only field-NPC object is B_EX110_RTN (objentry 0x84c, type 15).
   - Its motion set has only the idle (slot 0, 'R00A'). The talk task's motion 0x32 is absent, so he keeps idling (FUN_1403c88c0 failure path).
   - No vanilla field talk has Axel speaking. Three values are composed and marked in the config/code:
     - the Talk reaction (cmd 31 through the towns-person reaction sphere r 200);
     - the talk-camera shape;
     - the script: libretto op 1 with hb.bar 0x31f1 "Bingo. The name´s Axel. Got it memorized?".
2. **Reaction command (FUN_140407de0 / FUN_1404088f0).**
   - Sphere-vs-ellipsoid test: `(dx²+dz²)/(ra+rb)² + dy²/(ha+hb)² < 1`, using Sora's OBJ ellipsoid (r 35, h 55, centre 85 up). That gives about 209.8 units on flat ground.
   - Talk's +0x10 = cos 135° gates the angle; a failed gate shows the slot at RGB ×0.6.
   - cmd flags bit 4 blocks Talk in battle.
3. **WINDOW_TALK.** FUN_1403bb700: (256, 336), font 3, colour id 5, type 0, centred, tail group 47.
   - The box is the 'md_m' SEQD from fontinfo.bar, groups 67/68/69, on the world bar's 'md_m' art (HD PNG ×4).
   - The nine-slice is sized by FUN_140193430: interior = (pad + extent) & ~1, corners keep their size.
   - States: open → text → arrow (65) → advance on mask 0x40c with se000 #24 → next page or arrow 66 → close tail (48) → box close (69) (FUN_1403bce30).
4. **Text.**
   - Event font: 24×32 cells, 21 columns, reference height 24, spacing 2, glyph bottom at line + 2.
   - Advance is (int)(width × scale + 0.5); a space is 8 px; there is the 0x46 glyph rule.
   - Pages split at 0x10. Extents are the maximum over pages, rounded up to even.
   - The typewriter budget grows by 1 per frame; the default per-glyph cost of 0 shows a whole page at once.
   - Code lengths come from 0x1405c4cd0. Code 0x17 takes 2 parameters, which settles the arity question in `message.rs`.
   - Colour comes from 03system 'ftst' (header {1, count}, then 0x50-byte records): TT id 5 = 0x80050520.
5. **Talk task (FUN_140400f90).**
   - It locks Sora, who turns toward the NPC at his turn rate, and the NPC snaps to face him.
   - A 16-frame cross-dissolve leads to a 60° camera: eye = shape centre + r toward Sora, raised by h (FUN_140175e50).
   - At the end, facing and camera are restored with another dissolve.

## Implementation

- `sequence`: group padding fields and `Sequence::fit_nine_slice` (FUN_140193430).
- `windows_shell::dialogue` (new):
  - control-code lengths, line/page measurement, the text draw with its typewriter budget and glyph quads;
  - `TalkWindow` (constructor, state machine, draw order: box → tail → text → arrow);
  - a libretto op reader.
  - Tests cover code lengths, measurement, pages, typewriter cost and ops.
- `hud`: the event font (`load_font`), the window art (`load_window_sheet`) and `ftst_colour`.
- `command`: a reaction-command slot (group 2, dimmed ×0.6), placed above the rows [inferred row offset].
- `test_room`:
  - an `npc` config (model, idle, camera shape, messages, script, fonts, world);
  - the NPC's mesh and idle loop;
  - the reaction test with the Talk prompt;
  - the talk task: control lock, turning, NPC snap, script runner, windows with se000 #24;
  - the talk camera (FOV 60° converted like the follow camera's);
  - a snapshot-camera cross-dissolve under the HUD;
  - the talk-room unattended plan;
  - `test_room talk` reads `.local/test-room/launch-talk.json`.

## Evidence (private, outside Git)

`research/playable-room-20261008/autoplay-44` with config `test-room-talk-02.json`:
- The Talk prompt shows at tick 40.
- `talk_start` happens at 60, and the window opens for 0x31f1. Its size is 236×50, as the spec worked out.
- The advance at 150 plays se000 #24, and `talk_end` follows at 151.
- Captures 00 (prompt), 01 (dissolve) and 04 (close-up with the full window, tail and key arrow) show it; 06 shows the dissolve back.
- `autoplay-45` re-runs the combat room unchanged (hits 3/2/6, death, guard).

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS 180-frame smoke; PASS autoplay-44/45.

NOT RUN: a comparison with an installed-game talk capture (no vanilla Axel talk exists).

## Uncertainty

- **Composed (non-original):**
  - The Talk reaction for Axel.
  - The script.
  - The camera shape: the towns-person type-19 dimensions (r 90, h 30) on Axel's bone 100, the highest spine bone (about 177 units). The engine would fall back to a TARGET shape, which Axel's RTN model lacks.
- **Inferred:**
  - The reaction slot's vertical placement (0x179 less a pitch per row) and the text colour of that slot.
  - Battle = an enemy alive.
  - Gamma-space vertex colours are not modelled for the window.
  - The 16:9 centring is assumed because the window has no X origin.
  - Advance keys: Enter/K/LMB, Backspace and T (trigger bit 10 is untraced).
  - Sora's turn uses `Params::turn_speed`.
- **Not implemented:**
  - The reaction text's world/button icon codes (code 0x09 icon font).
  - Choices (mode 2), child balloons (code 05) and window type changes other than 0.
  - The party fade.
  - The RTN "after_talk" act.
  - Voice (field talk has none).
- **Timing:** unattended captures lag the requested ticks by a few frames in debug builds. The dissolve's per-tick alpha is logged by design, not by capture.

## Verdict / next

PROGRESS: field talk works end to end with the original's window art, font, layout, state machine and sounds, and Axel as a composed test NPC. Next: the icon codes in the reaction text, choices, and a vanilla towns-person talk (TT 0x46b) as a pipeline check.
