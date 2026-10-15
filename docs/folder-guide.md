# Finding and adding files

Start with the part of the game you want to change. These folders name what the
code does, so you do not need the task history to find it. This guide covers
editing the rewrite; a stable public mod API and mod package format are not yet
established.

## Choose a folder

| I want to change... | Start here |
|---|---|
| Movement, jumping or attacks | [game/player/src/lib.rs](../game/player/src/lib.rs), [combat.rs](../game/player/src/combat.rs) |
| Abilities, growth, magic or forms | `game/player/src/abilities.rs`, `growth.rs`, `magic.rs`, `drive.rs`, `forms.rs` |
| Shadow, Dusk or Soldier behavior | `game/player/src/shadow.rs`, `dusk.rs`, `soldier.rs` |
| Room setup, exits or progress | [game/area/src/](../game/area/src/) |
| Walls and floors | [game/collision/src/lib.rs](../game/collision/src/lib.rs) |
| Cutscene timing and camera state | [game/event/src/](../game/event/src/) |
| The launcher window | [main/src/launcher.rs](../main/src/launcher.rs), [launcher/](../main/src/launcher/) |
| The test room or debug menu | [main/src/test_room.rs](../main/src/test_room.rs), [debug_menu.rs](../main/src/test_room/debug_menu.rs) |
| Pause menu, equipment or journal | [visual/src/pause_menu/](../visual/src/pause_menu/), [pause_menu.rs](../visual/src/pause_menu.rs) |
| HUD, gauges or level-up panels | `visual/src/hud.rs`, `gauge.rs`, `level_up.rs` |
| Rooms, scenery or 2D drawing | `visual/src/room.rs`, `scenery.rs`, `overlay.rs` |
| Keyboard/controller mapping | [visual/src/input.rs](../visual/src/input.rs), [controller.rs](../visual/src/controller.rs) |
| Particle/model effects | [effect/src/](../effect/src/) |
| Mixing and Windows sound output | [audio/src/](../audio/src/) |
| Which sounds a scene requests | `visual/src/sound.rs`, `footsteps.rs`, `spell_sound.rs` |
| Reading a game file format | [data/src/](../data/src/); descriptive model, motion, room and table prefixes |
| Supported-source checks | [source/src/lib.rs](../source/src/lib.rs) |
| Model/image viewers or motion probes | [main/src/bin/](../main/src/bin/) |
| Build/research scripts | [tools/README.md](../tools/README.md) |
| What has been verified | [tasks/README.md](tasks/README.md), [progress/README.md](progress/README.md) |

## A few Rust words

- A **package** is a buildable unit. Its `Cargo.toml` lists its name and dependencies.
- `src/` means source code. Rust files end in `.rs`; `.wgsl` files are graphics shaders.
- `lib.rs` lists shared modules. A **module** is a named part of the code.
- `main.rs` starts a program. Additional programs live in `src/bin/`.
- Tests usually live beside the code they check or in a nearby `tests.rs`.
- **HUD** means the display over the game, such as health and command menus.

Readers sometimes keep the original format name for research: BAR is a container,
ARD describes areas, ANB contains motion, PTYA contains player action parameters,
SCD holds sound and IMGD holds images. These are formats, not bundled game assets.

## Names for future work

Use familiar nouns: `player`, `enemy`, `camera`, `menu`, `effect`, `audio`.
Use lowercase folders and lowercase filenames with underscores between words:
`pause_menu`, `new_game.rs`, `level_up.rs`. Name the feature or purpose rather
than its implementation technique. Avoid owned package suffixes `_sim`, `_vm`,
`_out`, unexplained initials and catch-all `misc` or `utils` files. Established
format names and upstream package names may stay for research and attribution.

Keep rules in `game/`, presentation in `visual/`, effect execution in `effect/`,
application wiring in `main/`, decoding in `data/`, validation in `source/`, and
mixing/output in `audio/`. An effect-file reader belongs in `data/`, running that
effect belongs in `effect/`, and drawing its output belongs in `visual/`.

Add a subfolder when several files form a clear feature. Add a package only when
working code needs an independently checked boundary. Do not scaffold empty
folders for future worlds or systems. Existing unfinished boundaries are listed
in [ARCHITECTURE.md](../ARCHITECTURE.md).

Move module declarations, imports, Cargo paths, asset/shader references, scripts,
ignore rules and current guides together. Run [the checks](../CONTRIBUTING.md#checks)
and record nontrivial work in `docs/tasks/`. Keep game data, manifests, saves and
captures in private storage outside the checkout.

## Names used by older records

Records through T128 retain the paths and commands actually used at the time.
Use these current names after `cargo ... -p`:

| Earlier name/path | Current package/path |
|---|---|
| `crates/windows_shell` | Programs: `main/` (`main`); shared display code: `visual/` (`visual`) |
| `crates/windows_shell/src/bin/kh2fm` | `main/src/`; launcher binary remains `KH2RW` |
| `windows_shell` smoke binary | `smoke` in package `main` (default run target) |
| `crates/player_sim` | `game/player` (`player`) |
| `crates/area_sim` | `game/area` (`area`) |
| `crates/field_collision` | `game/collision` (`collision`) |
| `crates/event_sim` | `game/event` (`event`) |
| `crates/effect_vm` | `effect` (`effect`) |
| `crates/audio_out` | `audio` (`audio`) |
| `crates/package_reader` | `data` (`data`) |
| `crates/source_identity` | `source` (`source`), scanner executable `source.exe` |
| `crates/skate_source` | `extras/skateboard/source` (package name unchanged) |
| `crates/skate3` | `extras/skateboard/engine` (upstream package names unchanged) |

Display module renames: `camp` to `pause_menu`, `pad` to `controller`, `bobs` to
`scenery`, `footstep` to `footsteps`, `newgame` to `new_game`, `levelup` to `level_up`,
`title_edits` to `title_banner`, `layout2d` to `layout`, `sprite2d` to `sprite`,
and `ui2d` to `overlay` (including its shader).

The old `cargo run -p windows_shell --locked -- --frames 180` is now
`cargo run -p main --locked -- --frames 180`. Launch with
`cargo run -p main --bin KH2RW --locked`. Inspector and probe binary names remain
the same. Source manifests, private launch configurations and saves are unchanged.
