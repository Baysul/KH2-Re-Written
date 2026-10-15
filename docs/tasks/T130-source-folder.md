# T130 — Group app folders under source and rename the validator

Owner Claude, 2026-10-15, requested by burba in the project thread. Gate P04
repository organization only. Prerequisites: fidelity contract, milestones,
unknowns, T128 and T129. Build identity: starts from `main` at `fab0e3b`
(T129 folder layout committed), branch `restructure/source-folder`, package
0.1.1, pinned Rust 1.99.0 Windows MSVC. Source identity: no installed-source
read, no gameplay admission, no game data touched.

## Steps and evidence

1. Committed the existing T129 layout first (`fab0e3b`) after checking the 739
   staged paths for game data and personal paths. None were found. Build passed.
2. Moved `main/`, `visual/` and `game/` into `source/` with `git mv`. Moved the
   installed-build validator from `source/` to `validation/`. Effect, audio,
   data and extras stay at the top level.
3. Renamed the validator package and its executable to `validation`. The
   workspace `Cargo.toml`, the lockfile (rename only, no dependency version
   changes), `source::` imports in 16 Rust files and the usage text were
   updated. Other crates keep their names.
4. Rewrote path dependencies with a script that resolves each relative path
   against the old location and writes it relative to the new one. Updated the
   workspace members and the `profile.dev.package` key.
5. Updated the default-deny `.gitignore` allowlist to the new paths. Checked that
   no source, manifest, shader or authored asset under the new layout is ignored.
6. Updated current guides: README, ARCHITECTURE, folder guide, tools README,
   validation README, effect README, developer tools, rust-rewrite priorities,
   and the two PowerShell helpers. The helper `Invoke-NativeMemoryReader.ps1`
   now looks for `target/debug/validation.exe`.
7. Left historical task records (T000–T129) with their original paths. The folder
   guide maps old names to current ones.

## Checks

- `cargo fmt --all --check`: PASS after `cargo fmt --all` re-sorted renamed imports.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo test --workspace --locked`: PASS, all suites.
- `cargo build --workspace --locked`: PASS.
- `cargo build -p main --features skate3 --locked`: PASS.
- `python -m unittest discover -s tools -p test_progress.py`: 9 tests, OK.
- `cargo run -p main --locked -- --frames 180`: the window opened and the run
  completed. The exit code was not captured, so this is not a strict PASS.

## Not done

- No per-file renames beyond T129. Names such as `data/src/probe.rs`,
  `data/src/vpu.rs`, `data/src/vif.rs` and `data/src/ogg.rs` still use format
  or research terms. Renaming them is a separate step.
- Not pushed and not merged to `main`. Waiting for the user to approve the merge.

## Uncertainty

- The lockfile was regenerated with a non-locked update, so its diff is the
  rename only. The rename is confirmed by the locked build afterwards.
- Windows CRLF warnings appear for the working copy. Content was not reformatted.

## Verdict

NOT RUN for the final merge; engineering checks above PASS on the branch.

## Next task

Approve or stop the merge to `main`. Then decide whether to do the per-file
renames listed above.
