# T123 — Remaining enemy motion and model boundaries

Owner Codex, 2026-10-14. Gates P10/P12/P13 inspection prerequisites for
P15/P19/P21/P27; no final gate advances. Prerequisites: fidelity contract,
milestones, unknowns and T122. User authorization: continue the local KH2FM
enemy, animation and summon-room work using REA/Ghidra. Build identity HEAD
`3aa251e`, package 0.1.1, with prior uncommitted T120–T122 work preserved.
Source: selected Steam app 2552430/build 15194255 English candidate remains
quarantined; supported edition/dependency identity is not established.

## Steps and evidence

Private evidence root:
`%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-continuation-20261014/`.
Prior REA/Ghidra evidence is retained under
`%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-native-20261013/`.

1. Reuse the REA identity and read-only Ghidra evidence; read T122's retained
   failures before altering readers. Installed files and saves stay read-only.
2. Fresh source-bound reads retain the failing motion/model originals in
   `source-01/`, with `admission.json` carrying private source identity and
   exact archive selection. Independently review the declared windows and
   stored matrix tables before accepting a second endpoint layout.
3. Extend the explicit stored-matrix inspection profile to retain both
   inclusive and one-past-the-last declared endpoints. Hold the final stored
   matrix at the latter boundary; no native interpolation or cadence claim.
4. Resolve the model EOF-link boundary against the pinned primary
   [legacy reader](https://github.com/OpenKH/OpenKh/blob/7a3b945c538d32c6a285128c98aefba093f52ceb/OpenKh.Kh2/Mdlx.Model.cs).
   Its empty next substream terminates iteration. Allow only exact EOF links,
   retain the raw link word, and reject partial/outside next headers.
5. Run generated regressions, independent source comparisons, the retained
   complete candidate audit, required workspace checks and release rendering.
   Record every unresolved record and deliver the verified local executable.
6. `raw-rust-01/report.json`, all private matrix buffers and
   `raw-oracle-01.json` independently agree on 27 raw motions, 14,430 animation
   matrices, 603 position matrices and 240,528 complete words, including every
   header word and first/final stored pose. Fifteen exclusive windows are
   preserved. Profile `raw_stored_matrix_floor_inspection_v2` makes the extended
   acceptance explicit; native timing/root semantics remain NOT RUN.
7. `model-fields-01.json` and `model-oracle-01.json` independently agree on
   one block, one group, three bones, 36 transform words and the full unknown
   region. Its forward link ends exactly at EOF. `pinned-Mdlx.Model.cs` and
   `primary-reader-receipt.json` retain the public reader observation; the full
   public reader was not executed. Geometry decoding passes but independent
   topology/render equivalence remains NOT RUN.
8. `corpus-03.json` completes fresh whole-source checks before and after the
   candidate audit, plus verified session reads. `corpus-03.exit.json` records
   exit 0. `corpus-summary-01.json` retains 304 records, 295 primary models and
   all 7,338 populated motion rows passing finite start/middle/end evaluation.
   Nine original archives have no model BAR member and remain FAIL; nineteen
   geometry-only material profiles remain UNQUALIFIED. No discovered row is
   removed, and no complete reachable inventory or native pose claim follows.
9. Extend the renderer tour to require four exact models, including the
   retained raw-motion candidate. No substitute actor can satisfy the verdict.
   Preserve primary linked-mesh omission warnings across status updates and
   distinguish additional archive models. Normalize one existing literal NUL
   to its Rust escape without changing its byte value.
10. `tour-delivered-02/` and its sibling JSONL/stdout/stderr/exit receipt
    pass through the updated release launcher: four exact actors, twelve
    source-slot selections, seven nonuniform captures and exit 0. Inspect
    captures 02 and 06: corrected raw slot playback, expanded arena, omission
    warning and actual four-actor summon list. This is renderer integration
    only; physical controls and installed comparisons are NOT RUN.
11. `release-build-01.log` passes with the existing `skate3` feature. Both prior
    executables are preserved and hash-checked at
    `%LOCALAPPDATA%/KH2FinalRewrite/backups/T123-launcher-20261014/receipt.json`.
    `launcher-receipt.json` binds built and delivered identities; the tour's
    exit receipt matches that executable. No commit, push or publication.

## Checks, uncertainty and verdict

PASS `cargo fmt --all --check`, warnings-denied workspace/all-targets locked
clippy, locked workspace tests (516 passing executions), locked workspace build
and the 180-frame shell smoke: `fmt-01.log`, `clippy-01.log`, `tests-01.log`,
`test-summary-01.json`, `build-01.log`, `smoke-01.log`. The smoke retains the
existing default controller-mapping warning. PASS independent finite raw/model
comparisons and complete audit execution; retain nine per-record model FAILs.
PASS progress generation/check and nine synthetic tests (`progress-tests-01.log`).
PASS release build, delivered identity and four-actor renderer tour. PASS
whitespace, changed-code and provenance review; no new dependency. All source-derived
payloads, pseudocode, addresses, captures and logs remain private, outside
Git. Partial static native slices do not count as complete functions.

Early audit launch attempts did not retain a usable report or exit result;
their empty `corpus-01`/`corpus-02` files are retained. A subsequent wait found
the prior process already absent. Correct the runner to pass explicit process
arguments, drain both pipes and await exit; only completed `corpus-03` counts.
These orchestration failures do not qualify source or decoder checks.
The first release tour fails before admission because its required empty
capture directory is absent (`tour-delivered.stderr.log`/exit receipt). Create
a new private empty directory and rerun as `tour-delivered-02`; no code repair
or hidden result change is involved.

Verdict: PARTIAL. The retained decoding boundaries and local delivery pass;
the complete requested enemy implementation is unfinished. Enemy AI, scheduler/traps, encounter activation,
context-dependent action selection, effects, linked presentation and installed
comparisons remain required. Next: qualify the remaining action/callback/trap
denominator and translate complete enemy behavior with multiple-actor targeting
and source encounter activation; resolve non-mesh presentation and
material/linked consumers, then run controlled installed/rewrite comparisons.
