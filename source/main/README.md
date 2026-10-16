# Application and launcher

`src/main.rs` starts KH2RW. `src/launcher/` checks the game folder and prepares
configurations. `src/test_room/`, `area.rs` and `cutscene/` assemble scenes.
`src/bin/` contains the smoke window and developer viewers/probes.

Launch: `cargo run -p main --bin KH2RW --locked`.
Smoke: `cargo run -p main --locked -- --frames 180`.

See the [folder guide](../../docs/folder-guide.md) and [architecture](../../ARCHITECTURE.md).
