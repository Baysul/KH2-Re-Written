# T131 — Move data, effect, audio and extras under source

Owner Claude, 2026-10-16, requested by burba in the project thread. Gate P04
repository organization only. Prerequisites: T129 and T130. Build identity:
starts from `main` at `e19e943`, branch `restructure/source-audio-data-effect-extras`,
package 0.1.1, pinned Rust 1.99.0 Windows MSVC. Source identity: no installed-source
read, no gameplay admission, no game data touched.

## Steps and evidence

1. Moved `data/`, `effect/`, `audio/` and `extras/` into `source/` with `git mv`.
   The `validation/` folder and the root `tools/`, `docs/` and `target/` stay where
   they are.
1a. Moved the effect crate again, to `source/visual/effect`, because it draws
   what the game produces. `audio`, `data` and `extras` stay under `source/`.
   AGENTS.md's placement rules now name the new folders.
2. Rewrote path dependencies with a script that resolves each path against its
   pre-move location and writes it relative to the new one. Updated the workspace
   members (`source/visual/effect`, `source/audio`, `source/data`, and the three
   `source/extras/skateboard/...` packages). The lockfile is unchanged, because
   workspace members have no paths in it.
3. Updated the default-deny `.gitignore` allowlist to the new paths.
4. Updated current guides and links: README, ARCHITECTURE, folder guide, CONTRIBUTING,
   tools README, developer tools, rust-rewrite priorities, CREDITS, and the moved
   READMEs' links. Fixed three `source/*/README.md` root links that the previous move
   had left broken. The link check finds no broken relative links in current docs.
5. Left in-game disc paths such as `data/audio/...` in the Skate 3 code unchanged. They
   are paths inside the disc, not repository paths.
6. Left historical task records (T000–T130) with their original paths.

## Checks

- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo test --workspace --locked`: PASS, 559 tests.
- `cargo build --workspace --locked`: PASS.
- `cargo build -p main --features skate3 --locked`: PASS.
- `python -m unittest discover -s tools -p test_progress.py`: OK.
- `cargo run -p main --locked -- --frames 180`: exit 0.
- Relative Markdown links in current docs: none broken.

## Not done

- Not merged to `main`. Waiting for the user's go-ahead.

## Verdict

NOT RUN for the final merge; the build check above passes on the branch.

## Next task

Approve or stop the merge to `main`.
