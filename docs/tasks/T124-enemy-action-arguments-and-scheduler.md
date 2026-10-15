# T124 — Enemy action arguments and scheduler research

Owner Codex, 2026-10-14. Gates P10/P12/P13 inspection prerequisites for
P15/P19/P21/P27. Prerequisites: fidelity contract, milestones, unknowns,
T106/T122/T123 and existing Shadow runtime. User authorizes continuing the
local KH2FM enemy/animation/summon-room work with REA/Ghidra. Build identity
HEAD `3aa251e`, package 0.1.1 with earlier uncommitted work preserved.
Selected Steam app 2552430/build 15194255 English candidate remains quarantined;
no supported build or final gameplay gate is qualified.

## Steps and evidence

Private root `%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-behavior-20261014/`.

1. Reuse the retained REA mixed-CLR/native identity and compatible read-only
   direct Ghidra workflow. Follow the REA skill's independent investigation
   instruction for Dusk source callbacks and native scheduler/traps. Installed
   files/saves stay read-only; all raw reports/listings stay outside Git.
2. `dusk/callback-graphs-01.json`, `callback-oracle-01.json` and
   `dusk-findings-01.md` preserve fifteen bounded source callback closures,
   covering 48 trap pairs, with raw opcode/operand and listing target/trap
   checks. Source Dusk motion requests with dynamic blending expose an
   avoidable loss of literal motion IDs in the current inspector.
3. Extend inspection with a bounded backward value-stack slice. Native motion
   trap argument order separates the logical motion cell from dynamic blend
   and optional frame cells. Preserve unchanged literals through supported
   stack effects; stop at calls, incoming control-flow joins, unsupported
   effects or budget. Do not execute helpers, select branches or evaluate
   runtime memory. Unknown logical requests remain in the denominator.
4. Freshly validate and reread selected Dusk/Shadow scripts outside Git;
   compare current bytes to the retained source before reusing old evidence.
   Qualify native action queue/start/update and trap signatures by independently
   matching both instructions and trap metadata to installed source bytes.
5. Test synthetic safety boundaries; audit all retained candidates, exercise
   newly exposed attack choices in the release renderer, and run required
   workspace/progress checks. Record all failures and unknowns honestly.
6. `source-01/admission.json` retains two uncached whole-source checks and
   verified session reads; `source-repeat-01.json` shows both freshly extracted
   scripts match T106 exactly. Source originals remain read-only. The first
   private helper compile fails on mismatched serde artifacts; the corrected
   helper emits primitive admission fields (`source-probe-build-02.log`) and
   reads successfully (`source-probe-01.log`). No reader change is needed.
7. `native/pass-{01,02,03}/native-audit.json`, `trap-tables.json` and
   `independent-source-01.json` independently match 34 bounded native slices,
   1,216 instructions/4,447 bytes and 480 exploratory descriptor rows. The
   sample does not prove complete trap-array membership. Exact executable/
   project/current-source identity and reported instruction bytes agree;
   the source remains unchanged. `scheduler-contract-01.md` retains the
   static contract and unresolved producers. `implementation-review-01.md`
   checks the implemented slice safety and native ring capacity against those
   receipts, plus the fresh script reread. No native function is qualified
   complete; opaque public candidate IDs map privately in `public-id-map-01.json`.
8. `dusk/candidate-comparison-01.json` independently matches all sixteen
   emitted Dusk request records against forward basic-block stack analysis
   (`forward-motion-oracle-02.json`). `forward-admission-02.json` retains 36
   roots/8,483 repeated source-word comparisons. Appearance and idle each keep
   two unresolved requests; other known values beyond the backward slice's
   supported effects remain unknown. This validates logical IDs only, without
   selecting native context or executing AI.
9. `corpus-01.json` and its stdout/stderr/exit receipt complete selected-source
   validation before and after the audit. `corpus-summary-01.json` retains
   304 candidates, 295 primary models and 7,338/7,338 finite sampled motion
   slots. Nine model archives still lack model entries. All 8,187 action
   records remain; literal requests rise from 4,958 to 5,365 (407 additional,
   zero removed), with 2,272 dynamic-action records and zero callback errors.
   The nineteen geometry-only material profiles remain unqualified.
10. Correct the existing Shadow queue to fifteen pending entries: the native
    sixteen-slot FIFO ring keeps one slot empty. Synthetic regression checks
    full capacity, order, start clearing and locked-action preservation.
    No general enemy VM or motion clock is enabled by this correction.
11. `tour-delivered-01/` passes four-actor rendering and selects Dusk source
    action candidates for claw, kick and walking attack. Its finite integration
    verdict passes, but visual review finds Dusk obscured by Sora in captures.
    Spread the authored tour placements around the player, retaining the first
    artifact as `runner-release-01.exe`. `tour-delivered-02/` and its sibling
    JSONL/stdout/stderr/exit receipt pass with four exact actors, twelve source
    selections, seven nonuniform captures and exit 0. Inspect captures 02 and
    04: the kick and walking-attack poses are now visible beside Sora.
    `tour-summary-{01,02}.json` binds selections and verdicts. Motion requests
    come from source named actions; the tour does not execute those callbacks.
12. `release-build-{01,02}.log` pass with the existing `skate3` feature.
    Preserve/hash-check both previous executables at
    `%LOCALAPPDATA%/KH2FinalRewrite/backups/T124-launcher-20261014/receipt.json`.
    `launcher-receipt-02.json` and the final tour exit receipt agree with the
    delivered `target/release/KH2RW Launcher.exe`. Source files, saves and
    existing launch selection/configuration remain intact. No commit, push or
    publication occurs.

## Checks, uncertainty, verdict and next task

PASS final `cargo fmt --all --check`, warnings-denied locked workspace/all-targets
clippy, locked workspace tests (520 passing executions), locked workspace build
and the 180-frame shell smoke: `{fmt,clippy,tests,build,smoke}-delivery.log`,
`test-summary-delivery.json`. Earlier complete final checks are retained under
`*-final.log`; the delivery checks include the visual-placement correction.
The shell smoke retains the default controller-mapping warning.

PASS fresh whole-source/script reread, independent Dusk argument oracle,
source-byte-matched native research and independent implementation review.
PASS complete audit execution; retain all nine per-record model FAILs and all
nineteen unqualified material records. PASS release builds, backed-up previous
executables, delivered identity and inspected final renderer tour. PASS progress
generation/check and nine synthetic tests (`progress-tests-01.log`), task-index
generation, whitespace/changed-code/provenance review. New repository content
is authored Rust, documentation and opaque public progress metadata; source
bytes, hashes, native addresses, pseudocode, captures and private receipts stay
outside Git. No new dependency or empty subsystem is added.

NOT RUN complete installed/rewrite enemy behavior and physical debug controls.
Full enemy AI requires the native scheduler, PC interpreter
adjustments, trap implementations, actor context, motion-end/range flags,
collision/hit identities and installed comparisons. Source callback decoding
and named animation candidates do not complete enemy behavior. Every required
unknown remains tracked; partial native slices do not count as complete
decompiled functions.

Verdict: PARTIAL. The additional animation mappings, queue-bound correction
and verified local delivery pass their finite checks; the requested complete
enemy implementation remains unfinished. Next: qualify native motion flag/
clock producers, interpreter adjustments and required trap membership, then
implement complete enemy actions with multiple-actor targeting/hits and source
encounter activation. Controlled installed/rewrite comparisons remain required.
