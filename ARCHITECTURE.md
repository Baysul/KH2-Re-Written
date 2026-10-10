# Architecture

Native Windows Rust/Bevy/wgpu rewrite of **Kingdom Hearts II Final Mix (PC) only**, reading the user's own Steam installation at runtime. Nothing from the game (assets, executable extracts, captures, saves, decompiled output) is stored in Git or distributed. Scope and gates: [fidelity contract](docs/fidelity-contract.md), [milestones](docs/milestones.md), [unknowns](docs/unknowns.md). Working rules: [AGENTS.md](AGENTS.md).

Status: the test room (walk, jump, two combos) and the title/camp screens run from the selected install. Fidelity is not established for combat rules, progression, saves or the full campaign.

## Layers

Data flows one way. Each layer may depend only on the layers above it.

1. **Source identity**: hash the selected install, write a manifest. Detection is evidence, not build qualification.
2. **Decoding (package_reader)**: bounded readers for BAR packages, models, motion, rooms, fonts, messages, audio. Every session re-checks the selected files against the manifest before reading a byte.
3. **Cache**: planned, transactional, bound to source identity. Not yet implemented.
4. **Authoritative simulation**: fixed-tick game state (`player_sim`, `effect_vm`). Renderer, window and input cannot change progression. Saves serialize versioned state.
5. **Presentation and adapters**: Bevy rendering, windows, audio output, pad/keyboard input (`windows_shell`, `audio_out`).

The rule is stated in AGENTS.md. Part of the current code does not yet follow it; see "Known gaps".

## Workspace crates

| Crate | Layer | Contents | Notes |
|---|---|---|---|
| `source_identity` | 1 | `lib.rs` (identity, hashing), `main.rs` (manifest CLI) | Read-only. Refuses existing output files. |
| `package_reader` | 2 | 44 modules in `src/` (BAR, plain/original payloads, message, font, model_*, motion*, room_*, scd, ogg, sequence, layout, objentry, preferences, ptya, battle_tables, effect); `src/bin/` 9 developer probes; `examples/` 5 boundary checks | Decodes; no gameplay rules. Probes and examples are research tooling. |
| `effect_vm` | 4 | `lib.rs`, `math.rs`, `programs.rs`, `mega_birth.rs`, `tests.rs` | Particle VM translated from the PC executable. |
| `player_sim` | 4 | `lib.rs` (locomotion state machine), `camera.rs`, `combat.rs`, `shadow.rs` | Test-room controller. Fixed-tick. Not a measured reproduction of KH2 rules. |
| `audio_out` | 5 | `lib.rs` (software mixer), `winmm.rs` (Windows output) | |
| `windows_shell` | 5, plus launcher and dev tools | See below | Largest crate; holds presentation and some game-state code. |

Dependencies: `windows_shell` uses all others; `package_reader` uses `source_identity`; `effect_vm` uses `package_reader`. Workspace lints forbid `unsafe_code`. Pinned toolchain: Rust 1.99.0 (`rust-toolchain.toml`). Bevy 0.19.1.

### windows_shell

- `src/lib.rs`: shared source-bound helpers for the developer binaries. Modules: `camp/` (abilities, customize, equip, items, party), `command`, `dialogue`, `effects`, `fade`, `gauge`, `hud`, `input`, `layout2d`, `newgame`, `pad`, `room`, `sound`, `splash`, `sprite2d`, `title`, `ui2d` (+ `ui2d.wgsl`).
- `src/main.rs`: `windows_shell` smoke app (synthetic checker BMP from `assets/development-checker.bmp`).
- `src/bin/kh2fm/`: **the player launcher**. `main.rs` lists programs (`title`, `room`, `talk`) and runs one with its launch file or arguments. `test_room.rs` is the playable test room. `title.rs` is the title screen presentation; its state machine lives in `src/title.rs`.
- `src/bin/room_scene.rs`, `model_inspector.rs`, `image_inspector.rs`, `motion_eval_compare.rs`, `motion_ik_probe.rs`: developer research binaries.
- `assets/`: `development-checker.bmp` (tracked). `glurbus-splash.png` is referenced by `splash.rs` but is git-ignored (see gaps).

## Where things are

```
crates/                      one folder per crate (layers above)
  source_identity/ package_reader/ effect_vm/ player_sim/ audio_out/ windows_shell/
docs/
  fidelity-contract.md milestones.md unknowns.md   scope and gates (paths fixed by AGENTS.md)
  tasks/                     one record per unit of work (T001-T085, plus T001R); format in AGENTS.md
  decisions/                 architecture decisions (0001-foundation.md)
  release/                   provenance, dependency inventory, third-party licences
  scenarios/                 scenario evidence format
  *-inspection.md, tool-*.md, source-*.md, supported-builds.md, ...   research and tool notes
tools/                       PowerShell, C#, Java and C++ research scripts (verify-*, native/managed readers, C++ references)
  fixtures/                  authored conformance inputs for those tools
.github/workflows/ci.yml     fmt, clippy, test, build on windows-latest
.local/                      git-ignored development outputs (see below)
```

Outside the checkout (private, never committed):

- `%LOCALAPPDATA%/KH2FinalRewrite/`: logs (`logs/`), research output (`research/<name>/`), and launch files `test-room-<name>.json` checked by the launcher. Files Claude writes under LOCALAPPDATA are virtualized for the user's own launches, so the test-room launch files go in `.local/` instead.
- Any directory the user names on the command line: source manifests, selections, configs, captures, saves.

## Local-only folders

- `.local/test-room/`: double-click launch files `launch.json`, `launch-talk.json`, `launch-title.json` (each names `steam_root`, `manifest`, `config`), plus source manifests (`source-manifest-0{1,2}.json`), test-room and arena configs, and a map-survey selection (`map-survey-01/`).
- `.local/tools/`: third-party clones and snapshots used for research (`openkh`, `requested-repos/`, and two other repositories not related to KH2; see the reorganization plan).
- `.local/*.json`, `.local/*.log`: generated metadata and smoke-run logs.
- `target/`: Cargo build output.
- `HANDOFF.md`, `gb_splash.png` (root): ignored working notes and a scratch copy of the splash image.

## Data flow

```
Steam install (read-only)
  -> source_identity  -> source manifest (private file)
  -> package_reader   (checks each read against the manifest; decodes in memory)
  -> windows_shell room/title/camp loaders (source-bound scene data)
  -> player_sim / effect_vm (fixed-tick state)
  -> windows_shell presentation (Bevy), audio_out (mixer), pad/keyboard input
```

Launch commands (examples; paths are placeholders):

```powershell
cargo run -p source_identity --release --locked -- '<install-root>' '<private>/source-manifest.json'
cargo run -p windows_shell --bin kh2fm --locked -- room '<install-root>' '<private>/source-manifest.json' '<private>/test-room.json'
cargo run -p windows_shell --locked -- --frames 180
```

Checks required after every change (AGENTS.md):

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

## Known gaps against the rules

1. **Game state inside the presentation crate.** `windows_shell` contains state machines for the camp menu, command menu, field talk, new-game dialog, title flow, gauge, fade and effect sources. AGENTS.md assigns game state to the authoritative layer. Move these when a test needs the boundary (see the plan).
2. **Two title modules.** `windows_shell/src/title.rs` (state) and `windows_shell/src/bin/kh2fm/title.rs` (presentation) split one screen. This is intentional for now; document it in the move.
3. **Private data in the checkout.** `.local/test-room/` holds source manifests, selections and captures. AGENTS.md says these belong outside the checkout.
4. **Unrelated game in `.local/tools/`.** Two clones belong to other games and do not serve this rewrite (AGENTS.md: KH2 Final Mix only).
5. **Ignored asset.** `splash.rs` loads `crates/windows_shell/assets/glurbus-splash.png`, but `.gitignore` allowlists only `development-checker.bmp`. A clean checkout cannot build the splash path.
6. **Stale doc.** `docs/architecture.md` describes three crates and says no decoder exists. This file replaces it once the plan's phase 2 runs.
7. **No cache layer.** Layer 3 is not implemented; nothing reads a cache.

## Adding things

- A new crate only when a tested behavior needs the boundary (AGENTS.md). No empty subsystems.
- Translated game routines must note the original function address in a comment or the task record. Assets stay out of the repo.
- Every nontrivial change gets a record in `docs/tasks/` with its gate, evidence locations and verdict.
- New `.gitignore` allowlist entries are reviewed for provenance before they are added.
