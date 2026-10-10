# Contributing

This repository rewrites Kingdom Hearts II Final Mix for Windows, reading the owner's own Steam installation at runtime. Read [AGENTS.md](AGENTS.md) for the rules, [ARCHITECTURE.md](ARCHITECTURE.md) for where things are, and [docs/fidelity-contract.md](docs/fidelity-contract.md) for what counts as established.

## Setup

- Windows. The Rust toolchain is pinned in `rust-toolchain.toml` (Rust 1.99.0); rustup installs it on first use.
- Put the Cargo bin directory (`%USERPROFILE%\.cargo\bin`) on `PATH`.
- Running the game needs your own legitimate Steam install of KH2 Final Mix. Nothing from it goes into the repository.

## Checks

Run these before each commit. Each must pass.

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo run -p windows_shell --locked -- --frames 180
```

The last command is the smoke run. Report a check you did not run as NOT RUN, not as passing.

## Making a change

- A nontrivial change gets a task record in `docs/tasks/`. The required fields are listed in AGENTS.md: ID, gate, owner, prerequisites, build and source identity, steps, evidence locations, checks, uncertainty, verdict and next task.
- Verdicts are PASS, FAIL, NOT RUN or BLOCKED. An unknown or skipped check does not pass a gate.
- Add a crate only when a tested behaviour needs the boundary. Do not add empty subsystems.
- Keep reading and decoding (`package_reader`) separate from simulation (`player_sim`, `effect_vm`) and from presentation (`windows_shell`). Renderer, window and input must not change progression.
- Translated game routines note the original function address in a comment or in the task record.

## Keeping private data out of Git

Never commit game data: assets, executable extracts, captures, saves, decompiled output, personal paths or secrets. Tests use generated synthetic bytes.

Private output goes in `.local/` (ignored) or outside the checkout. The default-deny `.gitignore` allows only listed paths. A new allowlist entry needs a provenance review before it is added. Check `git status` before staging, and check the file contents if anything looks like it could reveal a secret.
