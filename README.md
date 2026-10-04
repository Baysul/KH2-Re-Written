# KH2FinalRewrite

Native Windows Rust/Bevy research project for **Kingdom Hearts II Final Mix only**, using a user-selected local Steam installation. No game assets are distributed. This is a foundation under development; it cannot play KH2 yet.

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

This verifies the bound HED hashes and package metadata, retains unknown formats, and exports no original asset files. Encrypted/compressed and HD replacement readers are still under research.
