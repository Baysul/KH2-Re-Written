# T120 — REA identity, fresh native ability/growth/combo audit and Rust corrections

Owner Codex, 2026-10-13. Gates P10/P12/P18/P19/P22; no final gameplay gate passes. User request: understand abilities, growth and combo modifiers, show evidence, implement 1:1 in Rust, and explicitly use REA. Prerequisites: fidelity contract, milestones, unknowns, latest T119, existing T081/T090/T105/T108 ports. Build identity: audited checkout HEAD `1fc11a7`, package 0.1.1, plus this work. Selected source: local KH2FM Steam candidate, manifest reports app 2552430/build 15194255/English; fresh complete scan matches the existing candidate identity. It remains quarantined; vendor authenticity and complete supported edition/dependency profile are unqualified.

## Steps and exact private evidence

Private root: `%LOCALAPPDATA%/KH2FinalRewrite/research/abilities-20261013/`. No installed file was written and no target game was launched.

1. Read the required project records and inspect existing ability/growth/combo implementations. An independent scoped code audit found stale equipment-ability construction and missing consumers. Historical `playable-room-20261008` research directories cited by previous records were absent; do not treat those locators as freshly verified evidence.
2. REA tools were absent from this chat. Run the pinned REA 6.4.0 CLI. Doctor reported missing Codex registration and unset Ghidra environment. Set the existing Ghidra directory only for the command. Native analysis was rejected as `managed_target_unsupported` by the Windows P0 provider (`rea-equip-01.json`, `rea-equip-01.stderr.log`). No global registration, provider or installation changes were made.
3. Use REA's canonical managed inspection on the original file (`rea-managed-01.json`), Evidence `ev_ad0286c10649ad1fe055bca47488521179e02f0ecf80142e4e721c2cf5aac4f5`. It identifies mixed CLR/native and retains source digest/metadata locations. It did not observe gameplay or decompile the native ability functions.
4. Fresh complete selected-source scan: `source-manifest-01.json`. Final `ability_probe` uses an uncached binding to validate the complete source before/after reading, retaining seven original tables and exact package locators/digests in `tables-02/` (`probe-final.log`). `tables-01/` preserves the earlier session binding whose closing verification reused its admission; do not count it as a second full scan. This is an inspection route for quarantined input, not gameplay admission.
5. Direct Ghidra 12.1.4, existing source-matching project, `-readOnly -noanalysis`, authored `tools/AbilityNativeAudit.java`: `native-01` through `native-05` contain pseudocode and `native-audit.json` with instruction bytes/file offsets. The script requires source/project/expected SHA256 agreement, native language/compiler agreement, private output outside source/Git, and exact source-byte equality for every reported instruction. Source hash is checked again at completion. Changes to the project are discarded. First `.bat` launch failed on path parsing (`native-01.log`); direct Java bootstrap succeeded (`native-01-direct.log`). Other runs: `native-02.log` through `native-05.log`. Some requested slices are interior addresses rather than complete logical functions; use the specific qualified findings linked in the research document, not the number of selected slices as a coverage claim.
6. Independently read the complete 25-byte growth lookup from the file's PE section mapping (`growth-bits-01.json`); it matches the existing Rust lookup including zero entries. A separate file-byte comparison matches all 2,807 reported instructions / 11,339 instruction bytes over 47 requested slices (`independent-source-bytes-01.json`); this does not establish complete logical function bounds or an independent instruction-decoder oracle. Read the shared jump launch/entry/exit code to resolve Aerial Dodge's flag ordering; preserve its existing uncuttable behavior instead of applying an inferred fix.
7. Document interpretation and unresolved consumers in [abilities-growth-combos.md](../research/abilities-growth-combos.md), separating original observations from the port and remaining runtime questions.

## Implementation

- Equipment abilities rebuild from both member weapon slots, linked form weapon, armor and accessories, before equipped slots. Category-specific stat-key selection follows `1403e3ef0`. Equipment changes clear obsolete abilities; duplicate sources stack. The development room's second member weapon is empty; the state now represents it without inventing a save value.
- Member ability slots persist without a camp UI, preserving growth/reward changes through subsequent form changes and rebuilds.
- Boost damage uses explicit ability bits, branch priority and 32-bit wrapping arithmetic. Defender's accumulator is an unsigned byte. Square combo moves require contact even with Combo Master.
- The slot pass respects the native 80/24 record limits. FMAB rejects malformed nonzero pointers/truncated rows; this is reader validation rather than a claim about native corruption handling.
- Synthetic regressions cover equipment changes/unequipping, duplicate sources, form action/growth substitution and reversion, stat-key selection, boost-bit gating/priority/overflow, Defender wrapping, slot limits, malformed FMAB and the Square miss gate. No executable bytes or game-table fixtures enter tests or Git.

## Checks

- PASS `cargo fmt --all --check` (`fmt-final-02.log`).
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings` (`clippy-final.log`).
- PASS `cargo test --workspace --locked`: 480 passing test executions, including 120 player_sim tests and the three room equipment/form regressions (`tests-final-02.log`). Eleven new synthetic regressions; earlier runs retained separately.
- PASS `cargo build --workspace --locked` (`build-final.log`).
- PASS `cargo run -p windows_shell --locked -- --frames 180` (`smoke-final.log`, exit 0); this synthetic window does not compare installed gameplay. The controller library reported a default mapping; controller fidelity remains unqualified.
- PASS final source-bound probe and complete seven-table repeat comparison (`probe-final.log`, `review-receipt-01.json`).
- PASS diff whitespace and actual changed-code/fixture/provenance review; no new dependencies or private data added. `changed-code-hashes-01.json` retains the reviewed code identity.
- PASS native research/source receipts for their finite byte identities. REA native mixed-target analysis BLOCKED by provider capability; canonical managed inspection PASS. Installed gameplay comparisons NOT RUN.

## Uncertainty, verdict and next task

The ordinary ability construction and the selected growth/combo branches have fresh static evidence and Rust corrections. Full 1:1 remains unproved. Timing/window inclusivity, collision probes, installed root/pose behavior, the special-context level-five growth override, equipment/save population, all ability consumers, complete progression/save state and recorded reference scenarios remain open. Hyper Healing and Combination Boost have accumulated values without gameplay consumers; summons remain incomplete. No whole-game or full-system equivalence is inferred from these checks.

Verdict: PROGRESS. Next: qualify controlled installed/rewrite scenarios for each growth family, ground/air modifiers, miss/contact/finisher cases and equipment/form transitions; trace and implement the remaining ability consumers before advancing the gameplay gates.
