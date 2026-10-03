# T001 — P00 foundation, source inspection and engineering setup

Date: 2026-10-03. Owner: Codex with user ownership of edition/devices/save/release decisions. Objective: start P00 and do all authorized independent foundation work until reference evidence or consequential product inputs are required.

Prerequisites/authority: user's linked roadmap and explicit KH2FM-only/Steam-source clarification. Initial checkout `196af03` with README/license only. The final source code is the foundation Git commit containing this record. Candidate source is Steam app 2552430, manifest build 15194255, configured language english; exact content identity remains private. No gameplay-supported build is claimed.

## Work and artifacts

- Product/fidelity contract, source/release policy, supported-build status, architecture, unknowns, content/language ledgers, all 32 gate statuses and primary references under `docs/`.
- Rust 1.99.0 + Bevy 0.19.1 workspace: read-only `source_identity`, bounded `package_reader`, source-independent `windows_shell` smoke app. `Cargo.lock` pins dependencies; existing Apache-2.0 license retained. No installed bytes are copied into the workspace.
- Candidate scanner streams SHA-256, refuses missing/empty required files, rejects source-contained output/external links, binds Steam clues only when metadata names the selected folder, leaves candidates quarantined. Paths/timestamps are excluded from portable content identity.
- Package index and 16-byte header inspection follows pinned OpenKH structures; no payload export, name adoption, decryption or importer exists. Independent PowerShell field reader checks all six installed indexes.
- AGENTS, private-output ignore rules, Windows CI definition, pinned reference-tool audit and resolved Windows dependency metadata inventory.

## Checks and evidence

| Check | Result | Evidence / limit |
|---|---|---|
| Workspace native debug build | PASS | Cargo local build; not a playable rewrite |
| Formatting / warnings-denied Clippy | PASS after fixes | New Rust 1.99 lint changed fixed-size chunk iteration; no warnings suppressed |
| Synthetic source/index/header regressions | PASS, 11 tests | UTF-8/Unicode and long paths, relocation, incomplete/empty/damaged sources, bound Steam clues, SHA vector/read-only timestamps, output protection, signed/truncated/bounded records and package metadata |
| Native smoke: image/window/timed exit | PASS | `.local/smoke-first.log`: sample loaded, 180 frames, exit 0; final smoke recheck recorded locally |
| Native smoke rendered inspection / Escape | PASS | Computer Use observed imported checker on dark background; Escape logged clean exit; movement and controller calibration remain NOT RUN |
| User stopped Computer Use | OBSERVED | Physical Escape stop reported; no further Windows UI input was issued |
| Two full candidate source scans | PASS | External `source-manifest-release-before.json` / `source-manifest-release-after.json`: 13 files, 16,406,876,704 bytes; content/size/mtime identical; 133.98 s combined local optimized runs |
| Index / header installed sample | PASS structural checks | External `hed-inventory.json`, `hed-header-inventory.json`, `independent-hed-check.json`, `hed-final-inventory.json`; 46,076 records, no indexed range errors/overlaps/misalignment/within-index duplicates; see package report |
| Source unchanged across final package inspection | PASS | `source-manifest-release-after.json` and latest-scanner `source-manifest-post-inspection.json` match in content identity, hashes, sizes and modification times |
| Reference-game direct boot | FAIL | No usable window; working-directory retry exited -1073741819 (0xC0000005); user agreed to boot through Steam; cause unknown |
| Actual English/Japanese coverage / decoded payload equality | NOT RUN | No qualified sources/decoder corpus yet |
| Real game behavior / fidelity / save import / clean-machine release | NOT RUN | No gameplay implementation or reference scenario exists |
| CI execution / physical controller / secondary account | NOT RUN | CI authored; local pad mapping needs user calibration |
| Clean checkout and backup recovery | PASS | External bundle restored, offline pinned build/tests and its own synthetic asset smoke passed; see T001R for exact identity/limits |

External evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/foundation-20261003/`. Local build/smoke logs are `.local/`. Initial unoptimized multi-gigabyte scan was explicitly superseded by optimized scanner runs; it produced no candidate manifest and is not counted as a pass. Reads, fingerprints and structural checks support only the stated file subset; they do not prove clean vendor bytes or complete dependency/language coverage.

Verdict: foundation implementation and measured subchecks complete; P00-P05 final gates incomplete; preliminary P07 is inspection only. Next owner: user for a usable reference baseline/mod history/devices and scope choices, then Codex T002/T003. No public push or release is performed.
