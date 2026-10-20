# T174 — Choose the first Epic source target

Owner: coding agent with the project owner. Gates: P00/P02/P03/P06 planning only; no source or runtime gate advances. Prerequisites: fidelity contract, milestones, unknowns, folder guide, glossary, T173, Wayfinder/grilling/domain-modeling, the compatibility map, and its resolved source/build research children.

## Decision and identity

The user asked for plain English and for the major work to begin as soon as possible; save-file format compatibility can wait until that work is complete. Two live answers then confirmed:

1. Target the user's current Epic version first, keeping Steam working; other Epic versions come later.
2. Accept the simple plan of starting build/Linux work before save compatibility, checking selected files each launch, and withholding Epic support claims until real readers/current features are checked. Early Linux/Epic runs may have saving turned off, with a clear warning and existing saves untouched.

This resolves **Choose the source qualification and profile contract**. The initial four-question questionnaire was declined, not approved. In particular, its proposed three-state rejection model and per-feature admission options were not independently selected. The accepted source decision uses the existing quarantine boundary rather than presenting those unanswered alternatives as user choices.

Rewrite baseline: `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`, branch `epic-linux`; pinned Rust 1.99.0 and Bevy 0.19.1 unchanged. E01 is the historical T172 candidate, still unqualified. No installation, executable, vendor/rewrite save or game process was accessed in this session. No private fingerprints, bytes, executable details, captures or personal paths were added to the decision records.

## Steps and exact evidence locations

1. Claim **`.scratch/native_linux_epic/issues/05-source_qualification.md`** before investigation. Read the map, required project records, source/build research resolutions, Epic source-input audit and save-boundary observations. This is one HITL decision, not port implementation.
2. Record the user's priority and two live confirmations in that ticket. Correct earlier narration: archive similarity does not establish the same build/behavior; `Image/en` does not prove an internal language route; the pinned dependency is reachable upstream while its local cache is incomplete.
3. Append the accepted answer in **`.scratch/native_linux_epic/issues/05-source_qualification.md`** and mark it resolved. Required evidence includes source/language/package/reader identity, direct Rust reads of feature-required inputs, resolved duplicate selection and language closure, applicable compiled-rule/asset comparisons, per-session rejection and cache separation. These are retained safety/evidence obligations, not invented test expectations or proof that E01 passes them.
4. Distinguish early no-save runs from portable persistence and supported-source admission. The first path must not be held behind the known failing source-executable cipher/vendor-account dependency, but must truthfully disable persistence rather than pretend to save, reseed or overwrite progress. Existing Steam behavior/files remain preserved. Quarantine still applies; no-save mode is not a gameplay-admission bypass.
5. Update **`.scratch/native_linux_epic/map.md`** with the named decision link and staged priority. Keep the full save goal, but allow the first port handoff before save-format design. Add timing notes to **`.scratch/native_linux_epic/issues/07-portable_save_contract.md`** and **`.scratch/native_linux_epic/issues/09-progress_capture_restore.md`**; do not resolve them.
6. Narrow **`.scratch/native_linux_epic/issues/08-acceptance_matrix.md`** to the first no-save handoff and remove its portable-save blocker. It still needs the native-platform decision; later save acceptance follows the still-open progress/save questions. No-save trials are not complete feature parity.
7. Amend only the source/platform-extension and staged-save wording in **`docs/fidelity-contract.md`**. Preserve the historical Steam observations, measured comparison rules, unknown source qualification and final gates. Existing glossary distinctions between source compatibility, qualified sources, platform compatibility, feature parity and the two kinds of save remain unchanged; no new domain term is needed.
8. Verify documentation links, graph/state/index consistency, whitespace and public/private boundaries, then run the existing progress freshness and synthetic checks. No new tests, fixtures or debug scripts are retained. Code/Cargo/config/source/coverage remain untouched.

Decision evidence lives in the local ticket's `## Discussion` and `## Answer`, the updated map, and this record. Technical evidence is the already reviewed **`docs/research/epic_source_inputs.md`**, **`docs/research/build_verification_baseline.md`**, and **`docs/tasks/T172_epic_source_probe.md`**. T172 names its external private evidence locations; this session creates no new source evidence and does not reinterpret that inventory as an accepted source profile.

## Checks

| Check | Result |
|---|---|
| Live target and first no-save plan confirmation | PASS: two explicit answers; first questionnaire remains unaccepted |
| Source decision resolution and staged dependency updates | PASS: named resolution indexed; nine children, eleven acyclic edges, five resolved decisions; save questions still open; first acceptance blocked only by source/native-platform decisions |
| Local links, graph, public boundaries and whitespace | PASS: seven changed documents checked for local links, trailing whitespace and private-path/hash/address patterns; `git diff --check` passes; manual provenance review finds planning/contract text only |
| Existing progress freshness and synthetic tests | PASS: `python tools/progress.py --check` reports current; `python -m unittest discover -s tools -p 'test_progress.py'` runs all nine existing tests successfully |
| Cargo fmt/Clippy/tests/build and frame smoke | NOT RUN: documentation-only decision; prior dependency/build proof gap remains, no runtime result inferred |
| New/modified tests, runtime, Cargo, coverage or source/save files | None |
| Native Linux/Windows runtime, actual readers, source admission and comparisons | NOT RUN; E01 remains unqualified |
| Task index regeneration | NOT RUN: prior PowerShell availability gap retained; generated index not hand-edited |

## Uncertainty, verdict and next task

**PASS for the accepted human decision only; technical acceptance NOT RUN.** The local Cargo cache, direct Epic Rust reads, package/language evidence, native host behavior and controlled comparisons remain outstanding. Neither candidate stability nor disabling saves supplies those missing results. Existing coverage and final gates do not change.

Next: **Choose the native Linux platform contract**, then **Approve the compatibility acceptance matrix** for the first no-save implementation handoff. Actual build/Linux work remains downstream of this planning ticket; do not claim it happened here. Portable progress capture, formats, compatibility and migration remain open later work, not prerequisites to begin the main port work. No commit, push, publication or new test authorization is implied.
