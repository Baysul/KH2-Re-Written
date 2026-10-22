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
(decompiles `s1`..`s4`, the kn::Friend spec and decompiles in `friend/`); Ghidra MCP session on
a copy of the analysed project (private `kh2-command-deck/ghidra-mcp1`), then (from round (b)'s
`s4`, at burba's request) the headless runner `gh.cmd` there; summon script listings in the
private `kh2-command-deck/bdx`; unattended runs in the private `kh2-command-deck/runs/s1`..`s23`.

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

### Round (b): kn::Friend

- The friend object's update (`FUN_1401b15f0`) runs the personality's think (+0x380; default
  `FUN_1401b53b0`, Chicken Little `FUN_1401cb050`, Stitch `FUN_1401ca240`) and then the act
  tick (`FUN_1403b4f80`), and only while motion flag word +0x18c has 0x1c or bit 30 (the AI's
  free window). The think senses (`FUN_1401b8910`: Sora's block, the leash, a target by the
  personality's selector `FUN_1401b6b80` / `FUN_1401b3520` / `FUN_1401cbe50`), follows Sora or
  the target (`FUN_1401b41a0`, flight `FUN_1401b6360`, smoothing `FUN_1401b1b20`), and picks
  an action (`FUN_1401b1d40`; Chicken Little and Peter Pan "battle", Genie Follow and
  HealPlayer below a quarter of Sora's HP, `FUN_1401b3a30`), whose label starts an act
  unless a higher one runs (`FUN_1403caa90`). The friend keeps its choice in the object's
  target record (+0xbf8).
- Sora's attacks notify the friends (`FUN_1401b56e0` mode 1: the attack, its clock and its
  target; bit 9 of both friends cleared); `FUN_1401b5d60` ages the notice each frame and drops
  it when Sora's attack changes. `peterpan_receive_notify` (10:25, `FUN_1401cbdf0`) holds for
  8 frames until the friend takes it (10:26); Peter Pan's script then starts `attack_small`
  (within 300) or `attack_empty`. Hits on Sora go to an attacker list (mode 0).
- Chicken Little's fireworks (`P_EX360`, made by `sysobj_appear` inside his script) find him by
  entry (0x6c8), keep their effects on his watch list (10:53 `FUN_1401ce1e0`, 10:54
  `FUN_1401ce410`, 32 slots, ended effects swept by `FUN_1401ce050` each frame) and strike
  enemies within 240 of the burst (`attack_strike`, 2:17).
- An object in the ground mode that is off the floor and not rising while a gravity range
  holds (`FUN_1403b9090`'s leave-ground branch, `FUN_1403ba1e0`) falls: the class's handler
  ends in `FUN_1403b7850` (mode slot 4 with no height, `FUN_1403cbc30`, mode 1); the summon
  class first changes to motion 4 (`FUN_1403d6430`). Chicken Little's `surprise` lifts him on
  root motion and this brings him down.
- `btlobj_target` (2:23, `FUN_1404319d0`) searches again only for an enemy-class object
  (+0x6c8 bit 4) whose record kind is not 3 (`FUN_1403be2b0` kind 2 is Sora).

## What the rewrite does

- `test_room::summon`: the start, wait, placement, spawn and leaving above; the summon is a
  `ScriptBrain` running the object's own script with its own model and motion set (the
  gallery's loader and pose sampler), on Sora's team and of the summon class.
- `player::drive::Gauge::enter_summon`; `second_deck` shows Dismiss and routes Summon-list
  picks and Dismiss; `enemy::sysobj` now steps the object fade (+0xa08) each frame as
  `FUN_1403cc730` does.
- `--summon-tour <dir>`: each of the four summons with a full gauge [tour setup], the first
  three dismissed from the deck, the last running out of time; between the deck swaps Sora
  attacks (Attack every 20 frames) so that the summons that answer him have something to
  answer.
- `enemy::friend`: the kn::Friend controller above (think, sensing, following, selection,
  support pass, the watch list, Sora's attack notice) and `enemy::traps::friend`: table 7's
  friend traps and 35 of table 10's. `enemy::World::held` keeps the object the host runs and
  an object whose `sysobj_appear` is building another reachable to searches and handle
  lookups, as they are in the executable's single list. The summon object carries its
  objentry id; direct strikes on the enemy's stand-in land like the summon's attacks.
- `enemy::sysobj` ports the leave-ground fall; `btlobj_target` keeps the enemy-class test.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS |
| `cargo build --workspace --locked`, release `--features skate3` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Unattended `--summon-tour` (private `runs/s5`) | exit 0; all four summons start, appear, leave and end; Peter Pan ends by time |
| Unattended `--summon-tour` round (b) (private `runs/s23`) | exit 0; all four follow Sora; Chicken Little's fireworks strike the Shadow twice (ATKP 1261) and he falls back after `surprise`; Peter Pan starts `attack_small` on Sora's attacks; Genie waits on Sora's commands; Stitch stands (traps 10:1, 10:12, 10:32 missing) |

## Uncertainty

- [I] The first-time `appear` event is not played (the room takes the setting-Off path).
- [I] The summon is removed when its leaving fade ends: the removal's trigger in the kn::Friend
  update was not traced.
- [I] The summon level is 0 (no save is loaded); the team is Sora's (not traced in
  `FUN_1401b06e0`); the placement's wall line samples the room's wall push
  (`FUN_14040c2b0` not ported) and its last fallback is Sora's point.
- Stitch's think (`FUN_1401ca240`) and his traps 10:1, 10:12, 10:32 are not ported: he plays
  his appearance and stands. Genie's limit commands and forms, and the summon finishers, are
  not ported.
- [I] One Sora attack notice per Attack/AirAttack action change (Sora's +0x180 is not kept),
  aimed at the room's enemy while one is there; the notice ages once per frame after it is
  noted.
- [I] The friend watch-list sweep runs at the start of the friend's own update; the
  leave-ground fall takes collision as enabled (`FUN_1403ba940`), "on the floor" as within one
  unit of it, and sends no script event 0xe; the fall's arc is not frozen by the range that
  is still playing.
- [I] Direct strikes land at the stand-in's position with Sora's attack stats (the summon's
  sheet is not traced); hit callbacks of strikes are run as for volume hits.

## Verdict

PARTIAL. Summons come, last their time and leave as the executable sequences them; Chicken
Little, Peter Pan and Genie run their scripts under the ported kn::Friend controller and follow
Sora; Chicken Little and Peter Pan attack. Stitch, Genie's forms and the finishers remain.

## Next task

Stitch's controller (`FUN_1401ca240`, traps 10:1, 10:12, 10:32), Genie's limit commands and
forms, then the summon finishers (`limt` rows with members 200–204).
