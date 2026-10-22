# T202 — The second command deck (Summon, Party, Limit) and the world command layouts

Owner Claude, 2026-10-20, requested by burba in the project chat ("make the second menu
of the command deck the one with Party, Limit and Summon, and import every world-specific
command deck style, recreated 1:1"). Gate: development of the test room only; no final
gameplay acceptance is inferred. Prerequisites: command-01 (the field command menu),
T108/T113 (Drive, Limit Form), T186 (the party in the room). Build identity: branch
`claude/project-thread-6zu7ov` from `main` at `3645961`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build,
validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/command-deck-20261020/`
(spec `command-deck-spec.md`, decompiles `out/d1`..`out/d8`, data dumps `out/dd1.txt`);
layout probe output and the unattended run in the private `kh2-command-deck/`.

## What the executable does

- The command manager fills every menu each frame (`FUN_1403f29e0`). Menu 0 is the top
  deck (Attack, Magic, Items, Drive). Menu 6 is the second deck (`FUN_1403f3860`): Attack
  (`FUN_1403f2600`, as on the top deck); Summon (0x48, submenu 7) when a summon is offered,
  Dismiss (0x13c) while one is out, else the empty row 0x68; Party (0x49, submenu 8), or
  Genie's Drive row (0x25d) while Genie is out; Limit (0x51, submenu 9). The Party and
  Limit rows give way to the empty row in the restricted mode `DAT_1409ab580` bit 23.
- `FUN_1403f2470` swaps the decks: after the menu input (`FUN_1403b11e0`), logical bit 16
  (`MODE_FIELD`: D-pad left, or the right stick's left in its menu role) with only the top
  level up and the menu not locked (`FUN_1403b2780`) toggles mgr+0xbd4 between 0 and 6,
  keeps the cursor row when the other deck has it (`FUN_1403b1b80`), and plays system sound
  0x14. With a submenu up the same bit closes it (sound 4). Bit 28 (D-pad right) opens the
  row's submenu (sound 3).
- Drawing (`FUN_14015b6d0`, `FUN_14015ba00`, `FUN_14015bc30`): a swap plays the panel's
  swap group (`0x140718280`: 19/88/32 by style) before its loop and restarts the cursor;
  the pointer is group 21 beside deck 6 (20 beside deck 0) and hides while a submenu is up.
  The art set follows the battle style (`DAT_142a11404`); a style change replays the panel
  intro and flashes the cursor (`0x1407182b0`). Submenu art: Summon 73/70/72/71, Party
  36/33/35/34, Limit 56/53/55/54 (panel, cursor, row, last row).
- Lists: Summon (`FUN_1403e6d10`): the 00battle `sumn` rows whose charm is owned. Party:
  the two party slots' members, each with its command from the table at `0x1405c8fa0`.
  Limit (`FUN_1403e7b30`, `FUN_1403e81a0`): the 00battle `limt` rows whose participants are
  all in the field, whose world matches (0 = any), whose first participant holds its
  ability, and that are not a summon's; the first row of each id wins (so Sora alone gets
  the solo Trinity, and Timeless River gets its own Donald and Goofy rows).
- Colours (`FUN_140401d60`, `FUN_1403b6cf0`, `FUN_1403d88e0`, `FUN_1403b0f20`): flag bit 1
  commands are unusable in the air; limits cost all MP (orange while MP is short), are
  greyed outside a battle, while MP charges or a limit runs; a row whose list is empty is 5,
  one whose entries are all unusable is 1.
- World layouts (`FUN_140153ee0`, `FUN_140154070`): the menu loads
  `field2d/<lang>/<world><n>command.2dd`, where `n` is the room's 03system `arif` byte
  +0x34 (`FUN_1403a5920`), or the world-independent `zz0command.2dd` when the save's config
  bit 6 is set (`FUN_1402e5380`). The file is reloaded only when (world, n) changes, then
  the menu reloads without its intro (`FUN_14015db50`). The install holds 31 layouts (us):
  zz0, es0, tt0-tt6, di0, hb0-hb2, bb0, he0-he1, al0, mu0, po0, lk0, lm0, dc0-dc1, wi0,
  nm0-nm1, wm0, ca0, tr0, eh0-eh1. Every one names its pair `comw` and uses the same group
  numbering (98 or 99 groups), so each world's frames and colours come from its own file.

## Rewrite

- `visual::field_command_menu`: the two decks (`items`/`second`, `kind`), `Buttons` with
  left and right, `Decision::Toggled`, the swap animation and cursor restart, the battle
  style's art set and refresh flash, pointer 21 and its hiding, the Summon/Party/Limit
  submenu art, and `reload` for a new layout.
- `test_room/second_deck.rs`: the deck's rows, its three lists and their colours from the
  rules above, read from 00battle `sumn`/`limt` and 03system `cmd`/`item`.
- `test_room/command_layout.rs` and `area::command_layout`: the world layout follows the
  room; `--command-deck-tour <dir>` (`deck_tour.rs`) swaps decks, opens the three lists and
  then shows the second deck on all 31 layouts.
- Field input: D-pad left swaps the decks (sound se000 #20), D-pad right opens a row's
  submenu.

## Checks

- PASS `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D
  warnings`, `cargo build --workspace --locked`, the smoke run (`--frames 180`) and the
  release build with `--features skate3`.
- `cargo test --workspace --locked --no-fail-fast`: every suite passes except `enemy`'s two
  `traps::weapon` tests, which fail the same way on the base and are untouched here. New:
  `left_swaps_the_decks_and_keeps_the_cursor_row`,
  `execution_on_the_second_deck_reports_its_row`, `reads_summon_and_limit_rows`,
  `the_deck_is_attack_summon_party_limit`, `the_party_list_follows_the_slots`,
  `limits_need_their_members_world_and_ability`, `the_layout_follows_the_arif_digit`,
  `every_layout_gets_a_capture_after_it_loads`.
- PASS layout probe (`command_style_probe`, private output `styles.txt`): 31 layouts, each
  one `comw` pair; group counts 98 (es0, di0) or 99.
- PASS unattended `--command-deck-tour` on the selected source (private `runs/run1`, 41
  captures, exit 0): the swap at tick 100 plays se000 #20 and shows Attack, Summon (the
  tour owns the four charms; greyed, the room has no Drive bars), Party and Limit with the
  panel's swap group; the Summon list shows Chicken Little, Genie, Stitch and Peter Pan;
  Party shows Donald and Goofy; Limit shows Whirli-Goof, Knocksmash, Comet and Duck Flare
  (orange: MP short of all-MP; Sora does not hold Trinity Limit's ability in the room);
  left swaps back keeping row 3; then all 31 layouts load (`command_layout`, no errors) and
  draw the second deck in their own frames.
- NOT RUN: a capture of the original game to compare positions, timings and the button
  mapping; a room walk across worlds (the tour loads the layouts directly).
- No coverage-ledger change: no controlled installed/rewrite comparison was made.

## Uncertainty and what is not done

- Summons, the Party commands (execute 73) and the party limits (Comet, Knocksmash,
  Trinity and the world guests') do not run: an entry executed from a list closes the menu
  as the original queues the command, and is logged (`second_deck_command`). What execute
  73 does was not traced (the player class's handler at its vtable +0x138 was not found).
- The room's Donald and Goofy carry no ability record, so their limit abilities count as
  held; Sora's are checked. Summon entries take Drive's bar test outside a form; the
  player state bit +0x6cb and `FUN_1404005e0` are not modelled. A limit file counts as
  loaded (`FUN_1403e7e10`).
- The save's config bit 6 (the default command menu) has no switch in the room yet, so
  rooms always use their world's layout; arena rooms keep their configured file.
- The reaction command's world icon (`FUN_1401534b0` style 1..11 and `FUN_140153850`)
  is not drawn; the reaction slot still shows its label only.
- Right on a submenu row that has its own submenu (Cure's party pick) is not wired; decide
  works there.
- The physical mapping of logical bit 16 / 28 is read from the room's `MODE_FIELD` table;
  no capture of the original was compared.
- di0 (Destiny Islands) and es0 (world 1) draw parts of their rows with title art. Both
  belong to rooms without field play; whether the original shows them the same way was not
  checked.

Verdict: PARTIAL. The second deck, its lists, the swap and the 31 world layouts are in
the room as the executable builds them; summons, party commands and party limits do not
run. Next: the summon and party-limit actions, the reaction command's world icon, and a
switch for the default (zz0) command menu.
