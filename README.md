# KH2FinalRewrite

A native Windows Rust/Bevy rewrite of **Kingdom Hearts II Final Mix only**. It reads your own Steam installation at runtime. No game assets are distributed.

This is a foundation under development. It cannot play KH2 yet; a test room runs, and the rest of the game does not.

## Status

Sora walks, runs, jumps, lands and attacks (3-hit ground combo, 3-hit air combo) with his own animations and Kingdom Key, in a Twilight Town map. Everything is read from the selected Steam installation at startup. Poses use each motion's own IK skeleton and constraints ([T057](docs/tasks/T057-sora-ik-and-modes.md)). Sora walks in field mode with the Keyblade put away, draws it to attack, and attacks lunge forward with the motion's root movement.

Movement numbers come from Sora's installed parameter records. Several values (gravity, step height, combo order, body size) are explicit test-room choices, so the test room is not a measured reproduction of KH2's rules. See [T056](docs/tasks/T056-playable-test-room.md).

| Works now | Still missing |
|---|---|
| Checked package, model, texture, motion and room collision readers | Complete source/build/language support |
| Playable test room: walk/run by stick or Shift, jump/fall/land, ground and air combos, follow camera, wall/floor collision from the map's own surface flags | Enemies, hit detection, damage, sounds/effects, root motion during attacks, IK |
| Sora's walk/run speed, jump height, turn rate (PRTY), fall cap (SSTM), combo motions (PTYA), combo windows (ANB triggers), weapon joint (objentry) | Measured installed timing/physics; reference comparison (no Sora save yet) |
| Textured room and character; motion evaluation with IK helpers, leg/arm IK chains, constraints, limiters, expressions and root motion; Keyblade on the hand joint | Installed materials/lighting, culling, skeletal room props, HD routing; limiter damping, scale channels, root rotation |
| Synthetic platform window and repeatable checks | Combat, scripts, progression, saves and the full campaign/optional content |

Start with the [scope](docs/fidelity-contract.md), [milestones](docs/milestones.md), [unknowns](docs/unknowns.md) and [source policy](docs/research/source-and-release-policy.md). The external [roadmap](https://docs.google.com/document/d/1lewKD-h2ZVOOES1qugUgj82QVMybWwb8sqK3ed-4wPI/edit?tab=t.0) is the execution plan.

## Run the test room

One program, `kh2fm`, opens the title screen, the arena test room or the talk test room. Each launch checks the selected Steam files: the executable and package indexes are rehashed, and every package byte the session reads is verified against the full-scan manifest (schema 2, [T080](docs/tasks/T080-launcher-and-session-check.md)). It then decodes everything it needs. Selections and logs are private files outside Steam and Git. The configuration schema is in [T056](docs/tasks/T056-playable-test-room.md).

```powershell
cargo run -p windows_shell --bin kh2fm --locked -- room '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/test-room.json'
```

<<<<<<< Updated upstream
`title` and `talk` take the same three paths with their own configuration. To launch by double-clicking, build `cargo build --release -p windows_shell --bin kh2fm --locked` and create `.local/test-room/launch.json`, `launch-talk.json` and `launch-title.json` in the checkout (the exe also checks `%LOCALAPPDATA%/KH2FinalRewrite/test-room-<name>.json`) with `steam_root`, `manifest`, `config` (and optional `options`); `target/release/kh2fm.exe` then opens the launcher window: it finds the Steam install (or lets you browse or drop the folder), checks it against the supported build, remembers it in `.local/test-room/launcher.json`, and offers the Title Screen and the combined Test Room (the arena plus the talk config's NPC; `kh2fm play` from the command line) ([T089](docs/tasks/T089-launcher.md)). `--basic-fk` selects the older T053 FK-only poses for comparison.
=======
`title` and `talk` take the same three paths with their own configuration.
>>>>>>> Stashed changes

### Double-click launch

1. Build: `cargo build --release -p windows_shell --bin kh2fm --locked`
2. In the checkout, create `.local/test-room/launch.json`, `launch-talk.json` and `launch-title.json` with `steam_root`, `manifest`, `config` (and optional `options`). The exe also checks `%LOCALAPPDATA%/KH2FinalRewrite/test-room-<name>.json`.
3. Run `target/release/kh2fm.exe`, which shows a menu.

`--basic-fk` selects the older T053 FK-only poses for comparison.

### Controls

- Move: WASD or arrows (hold Shift to walk). Jump: Space.
- Attack: Enter, K or left mouse. Press again during a swing to continue the combo; attack in the air for the air combo.
- Camera: Q/E or right-mouse drag. Zoom: +/-. Collision overlay: F1. Exit: Esc.
- Options: `--gamepad` (left stick move, A attack, B jump, right stick camera), `--log '<new.jsonl>'` for an action log, `--autoplay '<empty-private-dir>'` for an unattended scripted run with captures.

## Developer tools

These need a selected install and a private output directory outside Steam and Git. Nothing they write belongs in the repository.

### Source identity

Write a candidate manifest to an **existing private directory**:

```powershell
cargo run -p source_identity --release --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json'
```

The scanner hashes only the observed KH2 candidate set, recording each file's SHA-256 and the SHA-256 of every 1 MiB chunk. It never labels an unknown build supported and refuses existing output files. Full shared/language dependency coverage and actual gameplay remain pending. Every session still checks the installation itself: a manifest only records what the bytes must be. Use the optimized scanner for multi-gigabyte packages.

### Developer room preview

```powershell
cargo run -p windows_shell --bin room_scene --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/room-selection.json' '<private-directory>/actor-selection.json' '<private-directory>/new-scene.jsonl' '<private-directory>/new-scene.png'
```

- J/L orbit the camera, +/- zoom, Escape exits. The preview captures a private PNG after settling.
- `--frames 900` gives a bounded capture run.
- `--motion '<private-directory>/motion-selection.json'` opens the basic FK motion preview (Space pause, N/B step, M next slot; `--tour motion` captures two frames per slot).

### Package probes

Unencrypted originals and BAR table metadata only:

```powershell
cargo run -p package_reader --bin plain_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-plain-report.json'
```

This verifies the bound HED hashes and package metadata, retains unknown formats, and exports no original asset files.

Bounded storage samples and their first two HD replacements:

```powershell
cargo run -p package_reader --bin original_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-storage-report.json'
```

The storage reader is checked against pinned OpenKH and .NET on finite samples. Source names, full dependency closure and semantic/gameplay decoding remain under research. Both commands retain only private metadata and hashes, and they require a full source rescan to establish mutation assurance. See [T005 evidence](docs/tasks/T005-storage-decoding.md).

Selected message and font structure:

```powershell
cargo run -p package_reader --bin message_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/selection.json' '<private-directory>/new-message-report.json'
```

The bounded selection names source identity and each index/ordinal/name hash plus a `message` or `font` research purpose. Message records retain encoded spans and hashes. Recognized localized RawBitmap records decode to indexed pixels, palettes and straight RGBA fingerprints. Glyph/Unicode mapping and in-game rendering remain unverified. An optional fifth argument names an existing private directory for new checkerboard font BMP previews, outside Steam and Git, with a 32 MiB aggregate cap. These are research assets and must not be distributed. See [T007 structure evidence](docs/tasks/T007-message-font-structure.md), [T008 pixel evidence](docs/tasks/T008-font-planes.md), and the [requested-tool prerequisite audit](docs/research/tool-prerequisites.md).

### Research notes

- [Static managed research](docs/research/managed-inspection.md): the pinned reader must pass an authored oracle before source-bound selections are inspected. Vendor bytes are parsed without execution. Findings and pseudocode stay private. This does not qualify native methods or enable gameplay.
- [Bounded static native research](docs/research/native-inspection.md): a separately pinned Ghidra engine must pass the current oracle and profile and preserve native PE mappings. Two getters and two startup-related bodies have finite independent instruction evidence. Full initialization, runtime behavior and gameplay remain unimplemented.
- [Pinned MCP bridge research](docs/research/bridge-inspection.md): finite fixture and source qualification with a private prepared-project workflow. Its generic importer cannot apply the required mixed-PE settings. The project database and all source findings stay external. Opening a bridge does not qualify gameplay.

## Build and check

Install the pinned Rust toolchain with rustup and the Visual Studio C++ build tools and Windows SDK, as in [Bevy setup](https://bevy.org/learn/quick-start/getting-started/setup/). Put `%USERPROFILE%/.cargo/bin` on PATH.

```powershell
cargo build --workspace --locked
cargo test --workspace --locked
cargo run -p windows_shell --locked
cargo run -p windows_shell --locked -- --frames 180 --log .local/smoke.log
cargo build --workspace --release --locked
```

The full list of required checks is in [CONTRIBUTING.md](CONTRIBUTING.md#checks). The smoke window imports a synthetic checker BMP, accepts WASD/arrows and Escape, and logs startup, sample loading, device axes and exit. Use `--gamepad` after checking the logged axes. An unrecognized local mapping reports idle drift. No game source is required for this smoke test.

## Private data

Source manifests, package records, captures and saves stay outside this repository. The smoke app's default log goes under `%LOCALAPPDATA%/KH2FinalRewrite/logs/`. Layout rules and private locations are in [ARCHITECTURE.md](ARCHITECTURE.md).
