# KH2 Final Mix rewrite

Read `docs/fidelity-contract.md`, `docs/milestones.md`, `docs/unknowns.md`, and the latest `docs/tasks/` record before work. User authorization: work through the roadmap from P00 using the local Steam files; KH2 Final Mix only. No other collection game belongs in this rewrite.

- Keep the installed source read-only. Never launch a writer/mod manager against it. Put manifests, extracted data, saves, captures and disassembly outside the checkout in private local storage. `.local/` is only for ignored development outputs; backups live outside the checkout.
- Require selected-source validation at every gameplay session. A remembered path, matching cache, Steam manifest or filename is insufficient proof of a supported build. Unknown builds remain quarantined.
- No game data, executable extracts, captures, user saves, secrets or personal paths in Git or distribution. Tests use generated synthetic bytes. Review provenance as well as ignore rules. Do not push or publish private research.
- Separate detection, decoding, cache, authoritative simulation and presentation. Create crates as needed; do not scaffold empty subsystems. Do not invent game rules or advance gameplay gates without reference evidence.
- Use `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, and `cargo build --workspace --locked`. Toolchain is pinned; ensure the Cargo bin directory is on PATH. Smoke: `cargo run -p windows_shell --locked -- --frames 180`.
- Nontrivial work requires an ID, gate, owner, prerequisites, build/source identity, steps, exact evidence locations, checks, uncertainty, verdict and next task in `docs/tasks/`. Use PASS, FAIL, NOT RUN, BLOCKED honestly. Unknown or skipped checks do not pass a gate.
- Resolve public technical claims against pinned primary sources and installed-build observations. A tool's README is not compatibility evidence. No claim of whole-game equivalence from finite opening tests.
- Preserve user files and saves. Do not switch Steam editions, repair/update the installation, spend money, share data or publish releases without the corresponding user instruction. Local inspection/builds are authorized.

## Changelog

When an agent finishes a job, add an entry to `CHANGELOG.md` under the next version (the one after the last release; the next version is `0.1.1`). Keep each entry to plain sentences: what a player or the user would notice, grouped under Added, Changed or Fixed. Do not change the version in `Cargo.toml` unless the user asks for a release.
