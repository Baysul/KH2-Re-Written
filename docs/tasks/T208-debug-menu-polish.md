# T208 — Debug menu: KH2-style panel, tabs, Help, motion and sounds

Owner Claude, 2026-10-23, requested by burba in the project chat ("start making the debug menu
look nicer, have animations snappy look, kh2 style menus, everything is named and labeled, a help
menu, and just looking better in general"), handed over by the project's coordinator session.
Gate: development of the test room only; the debug menu is not part of the original game and no
gameplay acceptance is inferred. Prerequisites: the F3 debug menu (PR #40) and its preview tabs
(T171). Build identity: branch `debug-menu-polish` from `main` at `6de4a1f`, package 0.1.2,
Rust 1.99.0 Windows MSVC, release built with `--features skate3`. Source: the selected Steam
KH2FM build, validated each run; the install was only read.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/debugmenu-polish-20261023/`
(unattended `--enemy-tour` runs `tour-01` and `tour-02`, captures `test-room-00..21.png`, logs
`tour-0N.jsonl`). No game data is in the repository.

## What changed

- Layout: the menu has four tabs, Sora, Enemies, Warp and Help (Tab / Shift+Tab, LB / RB, or
  Left / Right on a row with no value). Sub-pages (Items, Drive Form, Growth Abilities, the
  summon lists, Summoned Enemies, a world's rooms) show where they sit ("Sora / Items") in the
  strip; Back returns to the row that opened them. The Close row is gone; F3 or Esc / B close.
- Labels: rows sit under section headings (Status, Cheats, Items and forms; Battle enemy,
  Summon, Summoned; This world, Worlds; Play, Jump, Settings; ...). Every row shows its value
  where it has one (Lv, bonus number, On / Off, the active form, summoned count out of the limit,
  the previewed enemy, stock). The selected row's line at the bottom says what it does; the
  footer lists the keys and buttons of the page; long lists show "4 / 290" and a scroll bar.
  Rows that open a page carry an arrow; a selected value row has arrows either side.
- Help tab (also F2, or the pad's Select while the menu is up): menu controls, what each page
  is for, every field hotkey of the test room (F1 collision, F2, F3, F4 test blow, F5 / F6 HP,
  F7 / F8 MP, F9 Drive, F11 Drive refill, F12 stand-in party, J skateboard, Esc leave) and the
  keyboard game controls from the room's own ready line and `visual::input`. A unit test fails
  if a field hotkey is missing from Help.
- Look: a blue gradient panel with a light edge, a lighter wash behind the title, a tab bar and
  a cursor bar fading to the right, drawn on the HUD canvas with the system font. Text colours
  are the command menu's row, cursor and unusable-row text colours (0x140717d78 / 0x140717da0:
  0xb4, 0xe6, 0x64, 0x96 with 0x80 = 1.0); the panel colours are picked by eye to match the
  game's blue menus, not read from its files.
- Motion: open 0.12 s and close 0.08 s (slide in from the left and fade), a page or tab slides
  in from its side over 0.12 s, and the cursor and tab bar follow with a 0.035 s time constant.
  These are chosen values; the game's menu timings were not measured. Unattended runs draw the
  motion at rest.
- Sounds: the se000 menu sounds the pause menu requests (`pause_menu::SOUND_*`): open 3, cursor
  and tab moves 1, decide 2, back and close 4, an unusable row 5.
- Fixes in passing: J typed into a search no longer toggles the skateboard; the summoned-enemy
  rows read "Shadow No. 1" and the Warp row "Start here next launch: TT01" (the menu font has no
  parentheses, which drew as spaces); a stale "G refills" comment now says F6.
- The enemy tour captures the new pages after its earlier ones (Sora, Enemies, Warp, Help, the
  end of Help, Items, the preview's Enemy tab, an Items search).

## Checks

- Unit tests (`debug_menu`): tab wrapping on both strips, Back's parent pages and the row it
  lands on, header lines, the cursor passing over section headings (also when wrapping), page
  descriptions, every Help / title / hint / description line using only characters the menu
  font has, Help listing every field hotkey, the motion reaching rest within its times, and
  every draw staying inside the panel. A compile-time assertion keeps the list between the tab
  strip and the description.
- fmt, clippy (warnings denied), workspace tests, workspace build, release build with
  `skate3` and the 180-frame smoke run: see the PR.
- `--enemy-tour` against the installed build (`test-room-arena-07`, `source-manifest-02`):
  verdict pass, 22 captures. The first run showed grey, low-contrast text (colours at 1.0
  instead of the game's 0x80 scale), the cursor bar on a heading and a darkened band that
  stopped at the canvas edges; `tour-02` is after the fixes and reads cleanly on every page.

## Not checked / uncertain

- Keyboard and pad input in a live session (moving, tabs, F2, Select, Back to the opening row,
  typing, held-arrow repeat), the animations in motion and the sounds playing were NOT RUN: the
  unattended tour sets pages directly and draws them at rest, and no input was sent to a window.
- The Warp tab was only seen in the arena, which has no game room ("Warping needs a game room
  loaded"); its world list is the same code as before.
- Whether the On / Off of a growth row is the equip state is unchanged and not re-checked.

## Verdict

PARTIAL: layout, labels, Help, colours and the tour PASS; live input, motion and sounds NOT RUN.
Next task: drive the menu in a live session (keyboard and pad), and confirm the sounds and
timings feel right to the owner.
