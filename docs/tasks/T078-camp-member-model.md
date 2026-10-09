# T078 — Camp party frame member model (Tz::MenuObj)

Owner Claude, 2026-10-09. Gate P22. Starts from T077. User request: pause menu 1:1.

## Prerequisites / source

Private evidence: `research/playable-room-20261008/camp-03/menuobj-spec.md` (Ghidra, P_EX100.mset dump).

## Implementation

**`camp`:**
- `Sheet::Model(i)` draws: one whole-layout quad per member, tinted 0x60 (×0.75). Its alpha is the party frame's colour alpha (FUN_1402ffac0), and it lives only while the frame sprite does.
- The quad is inserted at prio 0x7538, after the lower bands and before the panel band.
- `model_offset(i, n)` (FUN_140306400): x is {47, 145, 243}[i] − (98n − 98)/n, y is 96.
- `model_time` (FUN_1403001b0) runs t over 1..=64. The clock starts when the top level is built and stops when the frame dies.
- The Pride Lands y of 61 is not used (this is a single-world room).

**`test_room`:**
- A second copy of Sora is rendered on layer 3 by an off-screen camera into a 1280×720 sRGB target:
  - horizontal FOV 40° (focal 879.19);
  - eye (0, 50, 800) looking at (0, 0, −55);
  - menu space (mx, my, mz + 100);
  - cleared transparent.
- The copy plays motion 0x8b, which is slot 6 (the field walk), with root motion kept.
- The camera is active only while the camp shows its members.
- The paste uses the full target with no clip to the frame: the feet hang below the frame edge, as the spec observes.
- The field camera query now excludes the menu camera. Without this exclusion, the field view stayed at the origin (autoplay-63).

## Evidence (private, outside Git)

**`research/playable-room-20261008/autoplay-64` (arena-06):**
- Capture 19 shows the top level with Sora walking in the party frame, toward the camera, with the labels and the Sora panel intact.
- Capture 20 shows the close.

**`autoplay-63`:**
- The failing run, with the field camera knocked out.
- Its captures 12 and 13 are 1×1 because the window was minimized. This is not a render fault.

**`talk-autoplay-02` (talk-03):** re-checks the talk room and the dissolve camera.

## Checks

PASS fmt; PASS clippy; PASS tests (new: member offsets and clock); PASS build; PASS smoke; PASS autoplay-64, talk-autoplay-02.

NOT RUN: comparison with installed-game captures.

## Uncertainty

- The model is lit and shaded like the field copy; the menu's lighting is not translated.
- Only Sora is drawn; party members' models and their 0x8b clips are not decoded.
- The sub-screen background copy (+0x1270) and the menu-space facing are inferred from the spec.

## Verdict / next

PROGRESS: the party frame shows Sora's walk. Next: Items / Abilities / Customize / Party sub-screens.
