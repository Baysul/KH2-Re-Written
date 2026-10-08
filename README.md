# KH2FinalRewrite

Native Windows Rust/Bevy research project for **Kingdom Hearts II Final Mix only**, using a user-selected local Steam installation. No game assets are distributed. This is a foundation under development; it cannot play KH2 yet.

Current implementation priority is a test room with controllable Sora, walking, jumping and attacking. A source-bound Bevy scene displays an actual textured room and a character with an explicit basic skeletal motion preview, reading their data from the selected Steam installation. Placement, materials and animation frames are inspection choices; it cannot play KH2 yet. See [T053](docs/tasks/T053-character-motion.md) and [T054](docs/tasks/T054-test-room-controls.md).

| Works now | Still missing |
|---|---|
| Checked package, model, texture and room collision readers | Complete source/build/language support |
| Room visual geometry with independently compared packet and mesh inputs | Installed rendering, culling, skeletal room props and HD material routing |
| Textured room and character with basic FK motion and retained skin bindings | Complete animation/IK, action selection, movement, camera rules and collision response |
| Synthetic platform window and repeatable checks | Combat, scripts, progression, saves and the full campaign/optional content |

Developer room preview (each launch revalidates the selected Steam files; all selections and outputs remain private):

```powershell
cargo run -p windows_shell --bin room_scene --locked -- '<selected-install-root>' '<private-directory>/source-manifest.json' '<private-directory>/room-selection.json' '<private-directory>/actor-selection.json' '<private-directory>/new-scene.jsonl' '<private-directory>/new-scene.png'
```

The preview supports J/L camera orbit, +/- zoom and Escape. It captures a private PNG after settling. Its source remains quarantined, and these inspection controls do not implement KH2 gameplay. Use `--frames 900` after the arguments for a bounded capture run. Add `--motion '<private-directory>/motion-selection.json'` for the checked basic FK profile: Space pauses, N advances a paused frame, B steps back and M cycles selected motion slots. `--tour motion` captures two explicit frames per selected slot. Missing IK, inverse curves, scale, constraints, expressions, root motion, triggers and installed cadence remain recorded. Movement, jumping and attacks require further reference work.

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
