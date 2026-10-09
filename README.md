# KH2FinalRewrite

Native Windows Rust/Bevy research project for **Kingdom Hearts II Final Mix only**, using a user-selected local Steam installation. No game assets are distributed. This is a foundation under development; it cannot play KH2 yet.

Current implementation priority is a test room with controllable Sora. **A playable test room now runs**: Sora walks, runs, jumps, lands and attacks (3-hit ground combo, 3-hit air combo) with his own animations and Kingdom Key, in a Twilight Town map, all read from the selected Steam installation at startup. Poses use each motion's own IK skeleton and constraints ([T057](docs/tasks/T057-sora-ik-and-modes.md)); he walks in field mode with the Keyblade put away, draws it when attacking, and attacks lunge forward with the motion's root movement. Movement numbers come from Sora's installed parameter records; several values (gravity, step height, combo order, body size) are explicit test-room choices, and it is not a measured reproduction of KH2's rules. See [T056](docs/tasks/T056-playable-test-room.md).

| Works now | Still missing |
|---|---|
| Checked package, model, texture, motion and room collision readers | Complete source/build/language support |
| Playable test room: walk/run by stick or Shift, jump/fall/land, ground and air combos, follow camera, wall/floor collision from the map's own surface flags | Enemies, hit detection, damage, sounds/effects, root motion during attacks, IK |
| Sora's walk/run speed, jump height, turn rate (PRTY), fall cap (SSTM), combo motions (PTYA), combo windows (ANB triggers), weapon joint (objentry) | Measured installed timing/physics; reference comparison (no Sora save yet) |
| Textured room and character; motion evaluation with IK helpers, leg/arm IK chains, constraints, limiters, expressions and root motion; Keyblade on the hand joint | Installed materials/lighting, culling, skeletal room props, HD routing; limiter damping, scale channels, root rotation |
| Synthetic platform window and repeatable checks | Combat, scripts, progression, saves and the full campaign/optional content |

### Playing the test room

Each launch rehashes the selected Steam files (about a minute), then decodes everything it needs. Selections and logs are private files outside Steam and Git; the configuration schema is in [T056](docs/tasks/T056-playable-test-room.md).

```powershell
cargo run -p windows_shell --bin test_room --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/test-room.json'
```

To launch by double-clicking, build `cargo build --release -p windows_shell --bin test_room --locked` and create `.local/test-room/launch.json` in the checkout (the exe also checks `%LOCALAPPDATA%/KH2FinalRewrite/test-room-launch.json`; the desktop app can virtualize that folder) with `steam_root`, `manifest`, `config` (and optional `options`, e.g. `["--gamepad"]`); `target/release/test_room.exe` then starts without arguments. `--basic-fk` selects the older T053 FK-only poses for comparison.

Controls: WASD or arrows move (hold Shift to walk), Space jumps, Enter / K / left mouse attacks (press again during a swing to continue the combo; attack in the air for the air combo), Q/E or right-mouse drag turns the camera, +/- zoom, F1 shows collision, Esc exits. Add `--gamepad` for a controller (left stick move, A attack, B jump, right stick camera), `--log '<new.jsonl>'` for an action log, or `--autoplay '<empty-private-dir>'` for an unattended scripted run with captures.

Developer room preview (each launch revalidates the selected Steam files; all selections and outputs remain private):

```powershell
cargo run -p windows_shell --bin room_scene --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/room-selection.json' '<private-directory>/actor-selection.json' '<private-directory>/new-scene.jsonl' '<private-directory>/new-scene.png'
```

The preview supports J/L camera orbit, +/- zoom and Escape, and captures a private PNG after settling. Use `--frames 900` for a bounded capture run and `--motion '<private-directory>/motion-selection.json'` for the basic FK motion preview (Space pause, N/B step, M next slot; `--tour motion` captures two frames per slot).

Start with [scope](docs/fidelity-contract.md), [milestones](docs/milestones.md), [unknowns](docs/unknowns.md), and [source policy](docs/source-and-release-policy.md). The external [roadmap](https://docs.google.com/document/d/1lewKD-h2ZVOOES1qugUgj82QVMybWwb8sqK3ed-4wPI/edit?tab=t.0) is the execution plan.

Install the pinned Rust toolchain via rustup and the Visual Studio C++ build tools/Windows SDK, as in [Bevy setup](https://bevy.org/learn/quick-start/getting-started/setup/). Ensure `%USERPROFILE%/.cargo/bin` is on PATH.

```powershell
cargo build --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p windows_shell --locked
cargo run -p windows_shell --locked -- --frames 180 --log .local/smoke.log
cargo build --workspace --release --locked
```

The development window imports a synthetic checker BMP, accepts WASD/arrows and Escape, and logs startup/sample loading/device axes/exit. Use `--gamepad` to enable controller movement after checking the logged axes; an unrecognized local mapping reports idle drift. No game source is required for this smoke test.

After selecting your collection install root, write a candidate manifest to an **existing private directory outside Steam and Git**:

```powershell
cargo run -p source_identity --release --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json'
```

The scanner hashes only the observed KH2 candidate set. It never labels an unknown build supported and refuses existing output files. Full shared/language dependency coverage and actual gameplay remain pending. Every eventual gameplay session will require the supported installation even when a cache exists.

Use the optimized scanner for multi-gigabyte packages. Detailed manifests and package records are stored outside this repository. The smoke app's default log goes under `%LOCALAPPDATA%/KH2FinalRewrite/logs/`.

Developer source probe (unencrypted originals and BAR table metadata only):

```powershell
cargo run -p package_reader --bin plain_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-plain-report.json'
```

This verifies the bound HED hashes and package metadata, retains unknown formats, and exports no original asset files. A second developer probe checks bounded storage samples and their first two HD replacements:

```powershell
cargo run -p package_reader --bin original_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/new-storage-report.json'
```

The storage reader is checked against pinned OpenKH and .NET on finite samples; source names, full dependency closure and semantic/gameplay decoding remain under research. Both commands retain only private metadata/hashes and require a full source rescan to establish mutation assurance. See [T005 evidence](docs/tasks/T005-storage-decoding.md).

Selected message/font structure inspection:

```powershell
cargo run -p package_reader --bin message_probe --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/selection.json' '<private-directory>/new-message-report.json'
```

The bounded selection names source identity and each index/ordinal/name hash plus a `message` or `font` research purpose. Message records retain encoded spans/hashes. Recognized localized RawBitmap records decode to indexed pixels, palettes and straight RGBA fingerprints; glyph/Unicode mapping and in-game rendering remain unverified. An optional fifth argument names an existing private directory for new checkerboard font BMP previews, outside Steam and Git, with a 32 MiB aggregate cap. These are research assets and must not be distributed. See [T007 structure evidence](docs/tasks/T007-message-font-structure.md), [T008 pixel evidence](docs/tasks/T008-font-planes.md), and the [requested-tool prerequisite audit](docs/tool-prerequisites.md).

For bounded developer inspection of the mixed executable's managed portion, see [static managed research](docs/managed-inspection.md). The pinned reader must pass an authored oracle before source-bound selections are inspected. Vendor bytes are parsed without execution; all findings/pseudocode remain private. This does not qualify native methods or enable gameplay.

For its selected native MethodDef leads, see [bounded static native research](docs/native-inspection.md). A separately pinned Ghidra engine must pass the current authored oracle/profile and preserve native PE mappings. Two getters and two startup-related bodies have finite independent instruction evidence, including the PE/CLR entry distinction; full initialization, active runtime behavior and gameplay remain unimplemented.

The [pinned MCP bridge research path](docs/bridge-inspection.md) has finite fixture/source qualification and a private prepared-project workflow. Its generic importer cannot apply the required mixed-PE settings. The project database and all source findings remain external; opening a bridge does not qualify gameplay.
