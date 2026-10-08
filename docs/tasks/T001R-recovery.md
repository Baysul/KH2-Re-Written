# T001R — P04 clean-checkout and backup recovery evidence

Date 2026-10-03. Owner Codex. Dependency T001 foundation commit `3135a9526f52cff0813567e7b930796357dc3b50`. Objective: prove the authored source and pinned dependencies can be recovered without `.local/` research, installed game bytes or private captures.

Created and verified an independent complete Git bundle outside the checkout under `%LOCALAPPDATA%/KH2FinalRewrite/backups/foundation-20261003/foundation.bundle`. Restored `codex/roadmap-foundation` from that bundle into `recovered-checkout` in the same external backup root. The restored checkout contains project source, synthetic BMP, lockfile and records, with no `.local/tools` or installed-source data.

Verification PASS: `cargo fmt --all --check`; `cargo build --workspace --locked --offline`; `cargo test --workspace --locked --offline` (11 tests); `git fsck --full`; restored source remained clean. Standard pinned Cargo registry/build caches were reused explicitly through `CARGO_TARGET_DIR`; no network or machine-local extraction tool was required. This is a fresh source checkout on the same machine, not a fresh-machine/toolchain test.

The restored application rebuilt against its own `CARGO_MANIFEST_DIR` and loaded its own generated BMP. Native recovery smoke PASS: 180 frames, `sample_loaded=true`, clean exit 0. Logs: `recovery-smoke.log`, `recovery-smoke.stdout.log`, `recovery-smoke.stderr.log` in the external backup root. Original source reads and user saves were not needed for recovery. Private `foundation-receipt.json` records the foundation commit and bundle SHA-256.

Verdict: recovery and clean-source build subchecks PASS. P04 user review, repository visibility and secondary environment remain unconfirmed; no push/publication occurs. The final backup additionally preserves the receipt/status documentation commit; source code is unchanged from the tested foundation commit. Next T002: establish usable reference baseline; T003: complete source/language closure and independent payload fixture before qualifying P02/P05/P06/P07.
