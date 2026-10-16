# T162 — NEW! badges: the Items row, new items from pickups

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub issue
#31). Gate: development of the test room only; no final gameplay acceptance is
inferred. Prerequisites: T073 (camp), T079 (camp Items), T161 (item boxes).
Build identity: `main` at `5c9cc05`, package 0.1.2, Rust 1.99.0 Windows MSVC.
Source: the selected Steam KH2FM build, validated each run; the install was
only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/levelup-20261016/`
(Ghidra decompiles `out/new/`, tour `badge1`) and the camp specs under
`playable-room-20261008/camp-0*`.

## What the executable does

- At camp start `FUN_140369fa0` zeroes a 0x11c-byte "new" mask object
  (`DAT_140bf0dc0`) and `FUN_140369520` fills it: an item of a counted type
  (inventory `FUN_1403c4720` non-zero) whose bit in the save's seen mask
  (`save+0x41a8`) is clear is new. Viewing a row sets its seen bit
  (`FUN_14036a1e0`); `FUN_140369f10` frees the object when the camp closes.
- Each top row asks predicates (camp-01 §3.3): Items `FUN_14036a8f0(−1)`
  decides the badge and `FUN_14036a7f0(−1)` (every new item seen) picks the
  dim (group 49) over the pulsing (group 50) art; Abilities
  `FUN_14036a5f0` / `a500` with `a3a0` and `a2b0`; Customize `aa70` / `a9a0`;
  Status `aba0` / `ab00`; Jiminy's Journal a task-start flag. The badge is a
  child of the row at the row group's (light X, text Y) = (100, 0), priority
  0x753d.

## Rewrite

- `pause_menu::build_top` adds the badge to the Items row from the camp's new
  stock items (the same predicate the Stock row already used).
- `pause_menu::items::new_items` computes the camp's new items; the test room
  seeds the starting stock as never new [I] and so only what the room adds
  (item drops, T161) shows NEW!.
- `--badge-tour <dir>` (unattended): a box lands on Sora, START opens the camp
  and DECIDE enters Items.

## Checks

- Unit test for the new-items rule; fmt, clippy (warnings denied), tests,
  build and the smoke run PASS.
- `--badge-tour` against the installed build: the Items row shows the
  pulsing NEW! badge after an item pickup, and the Items page's Stock row
  shows its own. Not compared with installed-game captures.

## Not reproduced / uncertain

- The Abilities, Customize, Status and Journal row badges, and the badges on
  the Abilities, Customize and Equip lists, are not built: they need the
  other new / seen masks (`+0x50`, `+0x5c`, `+0xa4`, `+0xc8` against
  `save+0x41f8`, `+0x4204`, `+0x424c`, `+0x4274`) and the learned-order lists
  that this room does not keep (the abilities list's badge sprite exists but
  stays blank).
- Which items the new-game save counts as seen is not decoded; the room's
  starting stock is treated as seen.
- Only the stock's counts decide new items; `FUN_140369690`'s per-category
  rules were not translated.

## Verdict

PARTIAL. Next task: the abilities, customize and status masks and their row
and list badges.
