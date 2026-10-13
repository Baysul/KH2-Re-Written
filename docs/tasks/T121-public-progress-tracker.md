# T121 - Evidence-based GitHub progress tracker

Owner Codex, 2026-10-13. Gate P04 reporting; no final fidelity gate advances.
User request: show percentage decompiled 1:1 and update the GitHub page repeatedly.
Prerequisites: fidelity contract, milestones, unknowns, latest T120 and the
existing GitHub workflow/provenance boundary. Build identity: checkout HEAD
`1fc11a7`, package 0.1.1, existing uncommitted T120 work plus these reporting files.
Source identity: documented Steam app 2552430/build 15194255 candidate; supported
edition/dependency profile remains unqualified. No Steam file or private research
payload was read by the generator; no reference game session was launched.

## Steps and evidence

1. Review gate and task evidence. Existing finite CIL bodies, native slices,
   decoded records, implemented features and successful tests do not qualify a
   complete logical function inventory or whole-game 1:1 denominator. All 32
   final roadmap gates remain unaccepted. Track T010's four historically selected
   managed pseudocode methods as PARTIAL using opaque public IDs; their function
   equivalence is unproved, and this record does not reverify private evidence.
   Interior native slices are excluded from the function denominator. Keep
   decompilation, controlled behavior comparison and roadmap acceptance separate.
2. Add `docs/progress/coverage.json`, a reviewed public ledger. Unknown totals
   produce JSON null and Unmeasured. Only PASS counts; PARTIAL, FAIL, NOT RUN and
   BLOCKED remain in the tracked denominator. Retain all P00-P31 gates. Initially
   no complete qualified functions or accepted behavior scenarios are entered;
   that is not a claim of zero research or zero implemented features.
3. Add an original Python standard-library generator, synthetic regressions,
   generated README section, detailed report and machine-readable summary.
   Validate schema, opaque IDs, duplicate units, evidence file boundaries,
   unsupported statuses, public label syntax and README markers. A human evidence
   review still establishes whether PASS or a complete inventory is justified.
4. Add GitHub push, daily 13:17 UTC, pull-request preview and manual triggers.
   Fork/PR validation has read-only permissions. Main refresh can commit only the
   three generated outputs, skips unchanged runs, rejects concurrent non-fast-
   forward updates and uses GITHUB_TOKEN to prevent repeated self-triggering.
   GitHub's primary documentation is linked in the generated report. Daily
   refresh does not collect new evidence or silently upgrade ledger verdicts.
5. Update AGENTS.md to keep coverage current with future reviewed work. Narrow
   ignore exceptions admit only the authored scripts and two public JSON files.
   Preserve all existing user/T120 changes; publish only this task's reporting
   changes. No installation, save, executable extract, capture, source identity
   digest, decompiler output or private research is included.

Exact local verification evidence: `%LOCALAPPDATA%/KH2FinalRewrite/research/progress-20261013/`:
`fmt.log`, `clippy.log`, `test.log`, `build.log`, `run.log`,
`progress-tests.log`, `progress-check.log`, `diff-check.log`.
Public outputs: `docs/progress/coverage.json`, `docs/progress/summary.json`,
`docs/progress/README.md`, the bounded progress section in `README.md`, and
`.github/workflows/progress.yml`. No new product dependency or game data is added.

## Checks

- PASS nine authored synthetic regression tests, including incomplete totals,
  zero versus unmeasured, regression, denominator retention, missing evidence,
  duplicate IDs, private paths, wrong game scope and deterministic generation.
- PASS `python tools/progress.py --check`; repeated generation makes no changes.
- PASS `cargo fmt --all --check`.
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- PASS `cargo test --workspace --locked`.
- PASS `cargo build --workspace --locked`.
- PASS `cargo run -p windows_shell --locked -- --frames 180`, exit 0. This is a
  synthetic window; its default controller mapping does not establish fidelity.
- PASS diff whitespace, changed-file review and reporting provenance review.
- PASS hosted push-triggered validation and refresh:
  [run 38475462537](https://github.com/BurBaGlurbus/KH2-Re-Written/actions/runs/38475462537)
  on reporting commit `6ebc5b9`. Both jobs succeeded; unchanged outputs were skipped.
- Automatic changed-output verification: add this record to P04's evidence links
  in the ledger and publish the ledger update without regenerating outputs locally.
  The next workflow must generate and commit the new report/ledger digest. This
  exercises the same refresh used by the daily schedule without changing any gate
  verdict. Record its result in the private delivery receipt after publication.
- NOT RUN future scheduled recurrence; the daily trigger cannot be observed in
  this session.

## Uncertainty, verdict and next task

PASS for local progress-system implementation and checks. Whole-game 1:1 remains
unmeasured; final roadmap acceptance is 0/32 (0.00%). A slice count, task count,
build result or equally weighted gate score is not a game decompilation percentage.
GitHub branch protections/token policy may block bot commits; the workflow reports
failure rather than bypassing protections. Normal GitHub scheduling can be delayed.

Next: qualify the complete KH2FM logical-function and required-scenario inventories,
record stable opaque IDs and reviewed acceptance evidence, and retain unknowns.
Continue the existing gameplay roadmap without advancing gates from this report.
