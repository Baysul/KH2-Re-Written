# T204 — Summons from the second deck

Owner Claude, 2026-10-20, requested by burba in the project chat (follow-on to T202/T203:
"then look at summons"; routines recovered with Ghidra MCP on the installed executable).
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T202 (the Summon list), T203 (limit engine), the `enemy` crate's
object-script engine, the enemy gallery's model loading. Build identity: branch
`claude/project-thread-6zu7ov` (PR #117), package 0.1.2, Rust 1.99.0 Windows MSVC, release
built with `--features skate3`. Source: the selected Steam KH2FM build, validated each run;
the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/command-deck-20261020/summon/`
(decompiles `s1`..`s3`); Ghidra MCP session on a copy of the analysed project (private
`kh2-command-deck/ghidra-mcp1`); unattended runs in the private `kh2-command-deck/runs/s1`..`s5`.

## What the executable does

- Execute 72 (`FUN_140405440`) for a Summon-list entry: Sora plays motion 0x8a (blend 8.0,
  0x140623bac), the command's cost (+0x1c, 3) is paid in Drive bars (`FUN_1403c2780`), and
  `FUN_1403de460` starts the summon: summon state `DAT_142a24edc` = 2, Sora held (player +0x120
  |= 0x104), Genie's form picked at random from the owned ones, and the task `FUN_1403de610`.
- `FUN_1403de610`: the party leaves (`FUN_1403e6fa0` → `FUN_1403e0460`), the task waits SSTM
  +0x88 frames (+0x84 for the first-time `appear` event, chosen by the summon effect setting
  `FUN_1402e4fa0`), the Drive gauge enters summon mode (`FUN_1403c2740`: mode 3, level + 3
  bars of SSTM +0x68 time; the level is `DAT_1409acdd6`, raised by `FUN_14040d530`), the
  `sumn` row's object (+4) is built as a kn::Friend (`FUN_1401b06e0`) of the summon class
  (+0x6c8 |= 0x400000) at a point 200–300 units from Sora (`FUN_1403f3fa0` /
  `FUN_1403f4180`: eight directions π/4 apart from a random base whose octant steps each call,
  a floor ray, a wall line, 100 nearer per round), Sora plays 0x82 and the object starts its
  act `appear_skip` (`FUN_1403cac50`). Sora is released and the state stays 2.
- Dismiss (execute 316, `FUN_1403de010`; also `FUN_140404a40` when the summon time reaches 0
  and Dismiss could run): state 3, gauge mode 1, task `FUN_1403de080` starts the act `leave`
  and waits for the object to go; the summon class's destructor (`FUN_1403dd850` /
  `FUN_1403dde30`) clears `DAT_142a24ee8` and the state; the party returns
  (`FUN_140400440`).
- The second deck's row 1 is Dismiss while the state is not 0 (`FUN_1403f3860`).
- The summon time drains in `FUN_1403a92c0` (× Summon Boost) and refills from hits ×12.

## What the rewrite does

- `test_room::summon`: the start, wait, placement, spawn and leaving above; the summon is a
  `ScriptBrain` running the object's own script with its own model and motion set (the
  gallery's loader and pose sampler), on Sora's team and of the summon class.
- `player::drive::Gauge::enter_summon`; `second_deck` shows Dismiss and routes Summon-list
  picks and Dismiss; `enemy::sysobj` now steps the object fade (+0xa08) each frame as
  `FUN_1403cc730` does.
- `--summon-tour <dir>`: each of the four summons with a full gauge [tour setup], the first
  three dismissed from the deck, the last running out of time.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `cargo build --workspace --locked`, release `--features skate3` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Unattended `--summon-tour` (private `runs/s5`) | exit 0; all four summons start, appear, leave and end; Peter Pan ends by time |

## Uncertainty

- [I] The first-time `appear` event is not played (the room takes the setting-Off path).
- [I] The summon is removed when its leaving fade ends: the removal's trigger in the kn::Friend
  update was not traced.
- [I] The summon level is 0 (no save is loaded); the team is Sora's (not traced in
  `FUN_1401b06e0`); the placement's wall line samples the room's wall push
  (`FUN_14040c2b0` not ported) and its last fallback is Sora's point.
- The summon scripts call 49 traps the engine lacks, most of them the kn::Friend controller
  (table 10: follow, targets, inertia, leave, Genie's forms, Stitch's and Chicken Little's
  shots); with them missing a summon plays its appearance and stands.

## Verdict

PARTIAL. Summons come, last their time and leave as the executable sequences them; their
behaviour while out (kn::Friend) is not ported.

## Next task

Port kn::Friend (`FUN_1401b06e0` and its update, the table-10 traps) so the summon scripts
run; then the summon finishers (`limt` rows with members 200–204) and Genie's forms.
