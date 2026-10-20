# Selected game source

`src/lib.rs` validates and hashes the selected files. `src/main.rs` is the
manifest command: `cargo run -p validation --locked -- <install-root> <private-manifest>`.
Every gameplay session validates the selected source. Cached names and remembered
paths cannot admit a build. Keep the installation read-only.

The selected files are the KH2FM executable and the six `kh2_*` package pairs of
one package folder: `Image/dt` (the Steam layout) or `Image/en` (the current Epic
copy's layout, [T180](../docs/tasks/T180_epic_source_binding.md)). A manifest
records which; package files in both folders are refused as ambiguous, and the
folders are never mixed. A folder name is a layout only: it establishes no
edition, storefront or language, and every candidate stays quarantined.

See the [folder guide](../docs/folder-guide.md) and [architecture](../ARCHITECTURE.md).
