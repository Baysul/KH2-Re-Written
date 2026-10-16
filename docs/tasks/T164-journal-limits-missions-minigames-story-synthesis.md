# T164 — Jiminy's Journal: Limits, Missions, Minigames, Story and Synthesis Notes

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub issue
#29). Gate: development of the test room only; no final gameplay acceptance is
inferred. Prerequisites: T093 (journal pages and notices). Build identity:
`main` at `c7aa907`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the
selected Steam KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/` — the pages'
routines, tables and groups are in `playable-room-20261008/journal-04/`
(`other-pages-spec.md` §0-§7, `puzzles-spec.md`) and `journal-01/`
(`jmemo-seq5-summary.txt`); unattended runs `jour1`..`jour6` under
`levelup-20261016/`.

## What the executable does

- The pages share one skeleton (container slide, breadcrumb pill, panel group,
  text rows, NEW label groups 57 / 58 and the single-row scroll list whose
  cursor is the first visible line).
- **Limits** (0x25 list, 0x26 detail): a limit is owned when its use counter
  (`save+0x3d48`) is above zero; the list shows owned limits (all of them,
  masked, under story flag 0x4837) as name and "N Hits" over a ten-row
  window; the detail page has the name and counter in a title strip over the
  description lines, R2 / L2 steps to the next / previous owned limit.
- **Missions** (10 / 0x23): the `ques` rows with a status 1 (open), 2 (done) or
  3 (masked under show-all) under world and category headers; the status comes
  from the row's flag bits, the story / menu flags and the minigame records
  (`FUN_14033c630`).
- **Minigames** (0xb / 0x24): the `mini` rows whose record state is above one
  (all under show-all) under world headers, each with its value as a time
  (`m'ss"cs`), a round, or a score with its label (`FUN_14033b940`).
- **Story** (3): the `stor` rows of the world whose flag is set, as one
  continuous column of text opening on the newest entry; an entry reaching the
  top row is marked seen.
- **Synthesis Notes** (0x27): four lines (Moogle level, materials, collection
  lists, recipes) with the totals 9 / 60 / 54 / 59, masked when only show-all
  reveals them.

## Rewrite

- `data::journal_table`: `read_quests` (`ques`) and `read_minigames` (`mini`).
- `visual::pause_menu::journal`: `limits.rs`, `lists.rs` (Missions and
  Minigames), `story.rs`, `synthesis.rs`; the Notes / Collection options now
  show these rows from the data, the new messages load with the journal's
  tables, and the NEW marks follow the seen sets the caller persists.
- `--journal-tour <dir>` (unattended) drives the camp to the journal and each
  page with test values (story flags 0x4837 and 0x8b0, the first four limit
  counters, some minigame records and a synthesis state).

## Checks

- Unit tests: the table readers, the mission status rules, the clock fields and
  the story line count. fmt, clippy (warnings denied), tests, build and the
  smoke run are listed in the PR.
- `--journal-tour` against the installed build: the Limits list and detail
  (with R2), Minigames, Missions, Story and Synthesis Notes pages all draw
  with the installed art and texts. Not compared with installed-game captures.

## Not reproduced / uncertain

- **Puzzles and puzzle pieces** (states 0x1b-0x21) are not built: they need the
  jigsaw board, the piece pictures and the placement rules (`puzzles-spec.md`).
- The scroll-bar thumbs of the lists are not drawn (the track is); the exact
  screen position of the NEW labels is [U] in the spec, so they sit at the
  spec's value / row anchors and overlap the value text of the Limits and
  Minigames lists.
- The room has no limit users, minigame records, missions or synthesis
  progress, so the pages show test values under the tour and are empty (hidden)
  otherwise; the save fields (`save+0x3d48`, `+0x3db4`, `+0x2390`, `+0x49f4`
  ...) are not modelled beyond what the pages read.
- The Missions masked rows keep the group's faint status icon (the spec says
  "without icons" in one place and group 0x6a in another).
- The Moogle names are system messages 0x3cd2..0x3cda; the table
  `0x1405bbd50` itself was not re-read.

## Verdict

PARTIAL. Next task: the Puzzles pages.
