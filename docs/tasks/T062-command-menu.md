# T062 — Field command menu and 16:9 HUD anchoring

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T061 (`09f2c76`). User goal: the HUD should look exactly like the original. Direct translation of the executable's routines into Rust is authorized; game assets stay out of Git.

## Prerequisites / source

Same selected Steam source and private manifest as T056–T061 (quarantined; full rehash at every launch). Ghidra 12.1.4 on the read-only installed executable, with private outputs in `ghidra-kh2-a3/cmd-01..08`. The written behaviour spec is `research/playable-room-20261008/command-01/command-spec.md`.

## Findings

1. The menu is `dk::COMMAND_DRAW`, which uses pair 0 of `field2d/<lang>/<world><n>command.2dd` (`tt0command.2dd`: one IMGD/SEQD pair named `comw`). It has a root sprite at (28, 322) (intro 7, loop 6). Each frame it rebuilds pooled `COMMAND_ONE` slots in call order:
   - the panel (17 → 18) at y = 388 − 22n;
   - the non-cursor rows (16, with 11 for the last row) at y = 410 − 22n + 22i;
   - the cursor row (9), the same item moved 8 right;
   - the icon frame 45, the category icon (37 Attack, 41 Magic, 39 Item, 44 Form) and the bouncing pointer 20, all parented to the cursor.

   Rows restart every frame. The cursor restarts only when it moves.
2. Labels are text drawn with the system font, not art:
   - Message ids are 0x81e0..0x81e3 (Attack, Magic, Items, Drive) from `msg/us/sys.bar`.
   - Font 1 is drawn at size 14 over an 18-pixel reference (scale 0.778), in 18×24 cells, 28 per row.
   - Glyph index = byte − 0x20, with widths from `fontinfo.bar` 'sys'. The advance is (int)(w × scale + 0.5); byte 0x01 advances a third of a cell.
   - Text sits at base + child context + (10, −8).
   - Colours by colour index are 0x80b4b4b4 for rows and 0x80e6e6e6 for the cursor; colour ≥ 4 is dimmed and cannot be selected.
   - The PC sheet is a 1024×512 HD PNG replacing the 512×256 'sys' plane.
3. Input: up/down wrap. Deciding executes the row or opens its submenu; after an execution the cursor returns to the top.
4. 16:9 anchoring: outside exact 4:3 render sizes, the 4:3 layout is centred in a 682.67-unit canvas. Elements are then pushed toward their nearest edge by 85: the command menu uses −85, and the gauge's +85 pushes it right. T061 drew the gauge without the 85.33 centring pad, so it sat 85 units too far left.

## Implementation

- `windows_shell::command`: COMMAND_DRAW translated for the top level in normal style. It covers the slot pool, rebuild and draw order, the cursor and its decoration, wrapping input, the cursor reset after execution, and system-font text quads.
- `windows_shell::hud`: system-font loader (HD 'sys' plane plus width table) and message-table reader.
- `package_reader::sequence`: `current_color` (the player's current animation colour, used for text alpha).
- `windows_shell::gauge`: the canvas adds the 85.33 centring pad, so the gauge is right-anchored.
- `test_room`:
  - An optional `menu` block with command, font image, font info and message locators.
  - The menu draws before the gauge. Up/Down arrows or the D-pad move the cursor (arrows no longer move Sora), and the attack button decides; Attack is the only command implemented.
  - Test-room choice: Magic and Drive are dimmed because the opening state has no spells or forms.

## Evidence (private, outside Git)

`research/playable-room-20261008/command-01/`: extracts, selections and reports, font previews, and the spec. Also `test-room-tt08-08.json` and `autoplay-27` (captures; log `autoplay-27b.jsonl`).

Inspected captures:
- The Twilight Town menu appears bottom-left: train/clock tower art, the COMMANDS tab, the Attack cursor with its Keyblade icon and pointer, Magic dimmed, Items, and Drive dimmed.
- After the scripted cursor move, Magic is under the cursor with the Magic icon.
- The gauge sits at the right edge.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (159 tests); PASS build; PASS 180-frame smoke; PASS autoplay-27; PASS release `test_room` build. NOT RUN: interactive playtest; side-by-side capture against the installed game.

## Uncertainty

Not implemented:
- submenus (Magic/Items/Drive lists), the battle-style art set (styles 1/2) and its hysteresis;
- the fade when Sora is behind the menu, reaction commands, the shortcut menu and sounds;
- the `zz0command.2dd` selection rule (the world file is used).

Inferred, not confirmed by a capture against the game:
- The ±85 edge anchoring and the full-window stretch of the 16:9 canvas.
- The text colour modulation (PS2-style ×colour/128 on the HD glyphs) and the exact text baseline.

## Verdict / next

PROGRESS: the command menu and the HUD anchoring now follow the original's code and data. Next: the remaining open items above, then the remaining parity areas (combat targets, submenus) as the user directs.
