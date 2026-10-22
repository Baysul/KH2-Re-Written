# T205 — One launcher executable

Owner Claude, 2026-10-22, requested by burba in the project chat: "when compiling it generates
a kh2rw.exe and it should only be KH2RW Launcher". Gate: build tooling only; no gameplay
gate moves. Prerequisites: T089 (the launcher). Build identity: branch
`claude/project-thread-pkr6gm`, package 0.1.2, Rust 1.99.0 Windows MSVC, release built with
`--features skate3`. Source: the selected Steam KH2FM build, validated by the launch check; the
install was only read.

## Cause

`source/main/Cargo.toml` names the launcher binary `KH2RW`, so Cargo writes `KH2RW.exe`.
`tools/Build-Launcher.ps1` (behind the four `build-*.bat` scripts) then copied it to
`KH2RW Launcher.exe`, leaving two identical executables. The game already runs inside the
launcher: each test restarts `std::env::current_exe()` with a program name (`title`, `room`,
`play`, `event`, `wm`, ...), and the world map's field start does the same. Nothing spawns
`KH2RW.exe` by name.

Cargo cannot name a binary `KH2RW Launcher`: bin names become crate names, which allow only
letters, digits and underscores (checked with a throwaway package).

## Steps

1. `Build-Launcher.ps1` moves the Cargo artifact to `KH2RW Launcher.exe` instead of copying it.
2. The launcher's missing-viewer message names only `KH2RW Launcher.exe`.
3. The ignored all-tests check (`every_test_starts_from_an_empty_data_folder`) runs
   `KH2RW Launcher.exe` when present, else Cargo's `KH2RW.exe`.
4. README-level docs that named `KH2RW.exe` as the product now name `KH2RW Launcher.exe`.
   Historical records and the folder guide's migration map keep their original spellings.
   `cargo run -p main --bin KH2RW` commands still work for development.

## Checks

See the verdict for results: `cargo fmt --all --check`, clippy with `-D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`,
`build-release-skate3.bat`'s helper (`tools/Build-Launcher.ps1 -Skate3`), the top-level
`.exe` list of `target/release`, opening the launcher, and running the title program through
the launcher executable with the arguments its Start button passes.

## Uncertainty

A plain `cargo build` still writes `KH2RW.exe` (and every probe); only the helper renames it.
The two viewers (`model_inspector.exe`, `image_inspector.exe`) stay separate executables the
launcher starts.

## Results

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` (one test ignored as before) and
  `cargo build --workspace --locked`: PASS.
- `tools/Build-Launcher.ps1 -Skate3` from an empty `target/`: PASS (14 min). Top-level
  `target/release/*.exe`: `KH2RW Launcher.exe`, `image_inspector.exe`, `model_inspector.exe`;
  no `KH2RW.exe`. A second, no-change run left the same three files.
- Launcher with no arguments: the window "Kingdom Hearts II Re:Written 0.1.2" opened on two
  of three starts. The first start exited within 8 s without captured output; it did not
  repeat. The run used the main checkout's private `.local/test-room` through a junction.
- `KH2RW Launcher.exe title <root> <manifest> <config> --frames 240` (the arguments its
  Start button passes to itself, from the private `launch-title.json`): validated the selected
  source, decoded the title, exit 0.

## Verdict

PASS for the build output: the helper leaves one launcher executable and the game runs inside
it. The ignored installed-game test was NOT RUN. No gameplay gate changes.

Next task: none queued.
