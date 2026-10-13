# Changelog

Every agent that finishes a job adds an entry under the next version. The next version is the one after the last released version. The last release is `0.1.0-alpha.1`, so the next is `0.1.1`. The version in `Cargo.toml` changes only when the user asks for a release.

## 0.1.1 (unreleased)

### Added

- Sound effects: footsteps from the floor under Sora, Sora's own cloth, jump and item-use sounds, the command menu's sub-menu open and close sounds, the room ambient loop, and spell cast and release sounds.
- Task records T115 to T117 in `docs/tasks`: the sound audit, footsteps and movement sounds, and the ambient bed and spell sounds.

### Changed

- Effect sounds play only for positive sound ids, at the element's start (Drive effects, `8a221ff`).

### Fixed

- Footsteps and movement sounds repeat on each walk or run cycle.
- The command menu no longer plays the dialogue's decide sound when a command runs.
- A refused item use is silent.

## 0.1.0-alpha.1

- First alpha: the launcher, the test room, and the build shown as alpha 0.1.0 in the launcher and `--version`.
