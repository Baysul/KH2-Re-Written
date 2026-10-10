# T074 — The test room on the exe's own pad and keyboard tables

Owner Claude, 2026-10-09. Gate P22. Starts from T073. User request: controller controls 1:1.

## Prerequisites / source

Private evidence: `research/playable-room-20261008/bgm-input-01/bgm-input-spec.md` Part B, `padmap-tables.txt` (mode tables), the base table 0x1405c3420 read from the exe (T071).

## Implementation

`test_room` now reads input only through `windows_shell::input` and `windows_shell::pad`.

**Devices to logical bits:**
- Keyboard, mouse and the first gamepad become PADDATA.
- The raw word follows FUN_14039bbe0, then the logical bits follow FUN_14039c720: pad mode 1 in the field, mode 7 while the camp is open.

**Field actions from new presses:**
- jump: 0x100, the back button (Space/Backspace, pad B/Circle);
- decide/attack through the command menu: 0x4 (Enter/left click, pad A/Cross);
- guard: 0x200 Square (right click/Right Shift, pad X);
- reaction/Talk: 0x400 Triangle (F/Right Ctrl, pad Y);
- talk advance: mask 0x40c;
- command menu up/down: 0x4000/0x8000 (arrows, D-pad, mouse wheel);
- camp: START 0x2 (M, pad Start).

**Movement:** the left stick's analog vector past the game's 60-unit dead zone. WASD deflects fully; Left Alt gives 96 (walk). The run threshold is now the exe's 0.8 (DAT_140585c7c), replacing the earlier test-room 0.6.

**Camera:**
- Logical bits 20/21 (yaw) and 18/19 (height, mapped to the follow camera's radius) are scaled by the right stick's magnitude, at the existing 0.05236 rad and 8 units per frame.
- R3 (bit 17: middle click/Right Alt, pad right-stick click) recentres.
- Mouse motion turns the camera (mouse-look stands in for the right stick).

**Debug keys and options:**
- The room's debug HP/MP/drive keys moved off the game's bindings to F5–F9.
- `--gamepad` is accepted but no longer needed.

## Evidence (private, outside Git)

- `autoplay-50` (arena-05): hits 3/2/6 at 418/447/486, death, guard with reflect 140, camp 1810–1936, exit 0.
- `autoplay-51` (talk-02): talk start, talk end at 151, exit 0.

Unattended runs script their input and bypass the devices, so these runs show nothing regressed; they do not exercise the new bindings. The bindings are covered by `pad` unit tests (title decide/START, the game's dead zone, the L2 right-stick role, repeat).

## Checks

PASS fmt; PASS clippy; PASS tests; PASS build; PASS smoke; PASS autoplay-50/51.

NOT RUN: hands-on play with a keyboard and a pad. NOT RUN: comparison with the installed game's feel.

## Uncertainty

- **Inferred:** mouse-look sensitivity and whether it needs a button; the gamepad name → glyph family.
- **Test-room mapping:** camera height maps to the follow camera's radius.
- **Not built:**
  - lock-on (R1, bit 11) and the shortcut menu (L1, bits 23–27);
  - first-person view (Select, bit 22);
  - the Japanese confirm layout option;
  - keyboard presets 2–6;
  - the PC settings overlay.

## Verdict / next

PROGRESS: field and menu input follow the exe's tables. Next: gamma-exact 2D composition, the camp's 3D party models, and music once the decoder question is answered.
