# T220 — Donald and Goofy in the camp: Customize, Abilities, Status, Items and Equip

Owner Claude, 2026-10-23, requested by burba in the project chat ("finish all summons, limits, and
party stuff until its 1:1"; "keep going"), with the coordinator session's brief: build the camp
Customize page for Donald and Goofy (battle style, ability and item frequencies) and their
Abilities screen.
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T215 (the members' character records), T218. Build identity: branch
`claude/project-thread-ww9y8z` from `main` at `c3d0932`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/`: `party-1to1-20261023/
party-camp-spec.md` (names, party list, new-game records, Status rows), `playable-room-20261008/
camp-07/customize-spec.md`, `camp-06/abilities-spec.md`, `camp-09/equip-spec.md` (headless Ghidra
reads of states 0x0f–0x21 and the Equip states); runs `kh2-party-1to1/runs/pc1`, `pc2`.

## What the executable does

- The camp's party list is the world's party table in order, Sora first (FUN_1402fc360); every
  screen reads the member's character record through its status object. Names come from
  0x1405c8f60 (0x8520 Donald, 0x8521 Goofy).
- Customize (FUN_14035b440 / FUN_14035bd90 / FUN_14035a4e0 / FUN_14035a180): a party member's
  categories are {Battle Style, Abilities, Auto-Reload}. Battle Style is one row (record +0xf4);
  Abilities lists the equipped action abilities in item-table order with their frequency (record
  +0xf5 + FUN_1401dc060); Auto-Reload lists the items with their frequency. The frequency glyph
  sits at each row's right end (FUN_14035add0, which ignores the list's scroll). △ steps the
  style (5 → 0), the ability frequency (Rare → Free), or the item through OFF+Often, ON+Often,
  OFF+Last hope!, ON+Last hope!. The popups have 6, 3 and 4 rows (the item popup draws each pair's
  frequency, FUN_14035af10); help appends the frequency's line. L2 / R2 switch members.
- Abilities (FUN_140360920 / FUN_140360f40 / FUN_14035ee10 / FUN_140361a00): a member's action
  abilities carry their frequency: an icon in the preview and the list, △ cycles Free →
  Well-Balanced → Rare → unequipped, and the popup has "Don't Equip" and three "Equip" rows with
  the frequencies. L2 / R2 step through the character rows (FUN_14035f130).
- Status rows for party bits 7..9: mask 0x1fc3f (no Drive Gauge, Bonus LV or form levels).
- Equip offers the member's own weapon type (pairs 0x1405bbe48: Donald staves, Goofy shields);
  slots come from the record (+0x10 armour, +0x11 accessories, +0x12 items).

## What the room does now

- Donald and Goofy join every character list after Sora: Items, Equip (their weapon type, armour,
  accessory and item slots from the new-game record), Abilities (their 80 slots, AP from the
  level-1 row), Status (their rows) and Customize (the party categories above, popups, help,
  glyphs, △ quick changes, the Auto-Reload warning, the cursor fix-up when a row goes away).
- L2 / R2 switch members in Abilities, Customize and Equip.
- A Party swap moves each member's pages with its name.
- When the camp closes, each member's record takes the changes (equipment, item slots and
  Auto-Reload, abilities, frequencies, battle style, HP and MP), and the AI reads them as before.
- The member record now holds its auto-reload originals, weapon, armour, accessories, bonuses and
  battle style; plrp row bytes 5..0xa are read for them.
- `--party-camp-tour <dir>` walks Donald's Customize, Abilities, Status and Equip pages, switches
  to Goofy with R2 on each, and checks Donald's record when the camp closes.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new tests for the frequency index, the plrp slot bytes, the party ability entries, the item cycle, the cursor fix-up and the tour schedule) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--party-camp-tour` (private `runs/pc2`) | exit 0, verdict pass: every press is answered (sounds 1/2/4 at their ticks); Donald leaves with Target Attack, Thunder equipped at Well-Balanced, Potion on Auto-Reload at Last hope!; captures show the preview, the list with glyphs, the 3- and 4-row popups, Goofy's list, Donald's Status (HP 18/18, MP 90/100, AP 5/5, Str 1, Mag 5, Def 2) and Goofy's Equip slots |

## Uncertainty

- [I] A member a Drive form holds out of the field stays in the camp's list (the camp's
  MENU_FRIEND objects stand for it).
- [I] The colours of the frequency texts the row callbacks draw (the argument is not decoded).
- Not built: the members' 3D models in the party frame and the Party cards (only Sora's model is
  rendered), writing a Party swap's order back to the room, the NEW! marks.
- Weapon changes in Equip are kept in the record but do not change the room's staff and shield
  or the members' battle numbers.
- Not compared: any page against the installed game's frames.

## Verdict

PARTIAL. The party pages run from the executable's rules and the room takes their changes;
nothing is compared with the original game.

## Next

T221: the battle styles in the AI (leash scale, the gate and pick tables, Party Attack's follow
and formation distances); then the Drive form effects the form-effects research lists.
