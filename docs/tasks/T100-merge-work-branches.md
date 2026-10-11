# T100 — Merge every work branch into main

Owner Claude, 2026-10-11. Gate: none (repository housekeeping; no gameplay gate advances). Starts from `main` at `5c6e271` (the current-state summary). User request: once every other thread had stopped, combine all the branches into `main` so the separate clones are no longer needed.

## Source

No game data was read for this task. Merged branch heads, each verified committed and pushed with a clean working tree and no stash in its clone:

| Branch | Head | Records |
| --- | --- | --- |
| `claude/project-thread-dhv9n6` (movement) | `bd797d9` | movement plan passes 2–4, T090 |
| `claude/project-thread-z71xi8` (combat) | `b108304` | T091, T092, T094–T096 |
| `claude/project-thread-b11b2c` (Twilight Town) | `f4bd620` | T097 (was T091) |
| `claude/project-thread-98hhql` (test room, pause menu) | `787bf31` | T093, T098 (was T090) |
| `claude/project-thread-f6il92` (cutscene) | `385e713` | T099 (was T090) |

The merge was done in a separate clone so the shared checkout was not touched until the result passed its checks.

## Conflict resolutions

- `bin/kh2fm/test_room.rs` (Twilight Town): the spell-volume and prize-marker pools keep `StandardMaterial`, through their own asset store beside Twilight Town's fog material; the area zone task runs after the magic and item ticks; the `Update` chain is split into two chained groups to stay under Bevy's tuple limit; the prize floor probe takes `RoomWorld`.
- `bin/kh2fm/test_room.rs` (test room): the unattended capture ticks are the union of both lists. The pause-menu picture source is shared through an `Arc`, as the Twilight Town loader now holds the source.
- `package_reader`: both path-hash modules (`name_hash`, `named`) are kept. The selection-validating model reader is renamed `read_selection_models` so the cutscene's public `read_models` keeps its name.
- Workspace, `windows_shell` dependencies, `kh2fm` programs and the launcher list keep both sides: `tt` and `event` programs, and four launcher tests.
- Task records renumbered: T091 Twilight Town → T097, T090 item pictures → T098, T090 cutscene → T099; T093's reference to the item-picture record follows. `docs/tasks/README.md` regenerated with `tools/New-TaskIndex.ps1`.

## Checks

On the merged tree: PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked` (every suite ok, none failed); PASS `cargo build --workspace --locked --release`; PASS smoke `cargo run -p windows_shell --locked -- --frames 180` (exit 0).

NOT RUN: the unattended test-room walks (`autoplay`) on the merged tree. The combat and pause-menu walks were written separately and now share one script, so their timings may interact. No interactive playtest.

## Uncertainty

- The two unattended walks were merged mechanically; whether the combined walk still reaches every screen it captured before is unverified.
- `name_hash` and `named` both hash archive paths; they could be folded into one module later.

## Verdict / next

PROGRESS: every branch is on `main` and the workspace builds. Next: run the combined unattended walk, and the owner's decisions on the old clone folders and on `docs/research/movement-feel-1to1-plan.md`.
