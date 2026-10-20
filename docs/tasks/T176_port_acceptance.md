# T176 — Approve the first port acceptance checklist

Owner: coding agent with the project owner. Gates: P00/P03/P30 planning only; no source, runtime or fidelity gate advances. Prerequisites: fidelity contract, milestones, unknowns, folder guide, glossary, T175, Wayfinder/grilling/domain-modeling, compatibility map, accepted source/native-platform decisions, and the source/host/build research records.

## Decision and identity

The user asked to proceed after the native Linux plan. They were shown six plain-English input/result checks: unchanged pinned build plus existing checks/180-frame smoke; native Wayland launcher/helpers with Windows preserved; actual graphics/sound/input evidence; qualified real Steam/Epic readers and fresh source rejection despite a cache; source/cache separation; and truthful no-save operation with existing saves/Steam behavior preserved. The proposal kept Windows/Linux × Steam/Epic results separate, missing evidence BLOCKED/NOT RUN and permission to start coding separate from completed support. The user selected **Accept checklist (Recommended)**.

This resolves **Approve the compatibility acceptance matrix** and finishes the first-stage planning handoff. The complete accepted checklist is [preserved below](#accepted-checklist); T194 retires the temporary scratch tracker. Later portable-progress/save questions remain open and deferred; they do not block implementation. This is not approval of new tests, numerical tolerances, reference settings, source qualification or complete feature parity.

Rewrite identity verified using `git rev-parse HEAD` and `git branch --show-current`: `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`, `epic-linux`, with pre-existing uncommitted planning/research work. Rust 1.99.0, Bevy 0.19.1 and the existing dependency revision/lock remain unchanged. The prior Linux inventory is not re-measured; E01 remains unqualified. No installation, executable, extracted output, save, game application or GUI was accessed or changed.

## Steps and exact evidence locations

1. Claim **`.scratch/native_linux_epic/issues/08-acceptance_matrix.md`** before investigation. Read the map and required project records; consult the interview/domain-modeling skills and established source/persistence/feature-parity distinctions.
2. Review **`docs/research/linux_host_boundaries.md`**, **`docs/research/build_verification_baseline.md`**, **`docs/research/epic_source_inputs.md`**, existing-check declarations and the accepted source/platform answers. These supply the build, native-host, reader, save-startup and physical/reference evidence boundaries, not new runtime proof. No new external API research is claimed.
3. Present the short checklist and obtain live approval. Append `## Discussion` and `## Answer` to the acceptance ticket; keep four required platform/source cells and concrete existing/manual acceptance actions, truthful partial/blocked results, private evidence/provenance rules and the approved no-save exception. Retain fresh per-session admission and real-reader qualification; no-save startup is not an admission bypass.
4. Preserve all existing tests, the pinned baseline and source/save files. The answer expressly requires separate approval before any new/modified tests or fixtures, and preserves existing controller/save tests without pretending they qualify physical devices or portable persistence.
5. Add the named resolution to **`.scratch/native_linux_epic/map.md`** and mark the first-stage handoff ready, with implementation next outside the planning map. Leave the two later save tickets open and their dependency intact. Existing observation-dependent hardware/data fog remains; no new sharp planning question emerges. Do not close the overall map or auto-claim the deferred save frontier.
6. Add the narrow accepted-checklist pointer to **`docs/fidelity-contract.md`** without changing comparison tolerances, final gates, historical source evidence or required coverage. No glossary change is necessary; no runtime, manifests, lock, tests, ledger or progress report are changed.
7. Check the four changed documents, tracker state/links/dependencies, whitespace/public-private boundaries and existing progress freshness/synthetic suite. Do not launch applications, install packages, modify global settings, trigger remote CI or publish anything. Native/hardware/source comparison execution remains downstream.

Exact decision evidence: the acceptance ticket's `## Discussion` and `## Answer`, its map entry and this record. Technical evidence remains the linked research records and their primary-source pointers. No private evidence directory, source fingerprints, addresses, bytes, captures, hardware identifiers or personal paths are introduced.

## Checks

| Check | Result |
|---|---|
| Live checklist approval | PASS: explicit selection; no new-test or save-format authorization |
| Rewrite identity | PASS for unchanged rewrite HEAD/branch only; selected-source validation NOT RUN |
| Resolution, local links, index and dependencies | PASS: four documents, 33 local links, nine children, eleven acyclic edges and seven indexed resolutions; both later save tickets remain open/unassigned with their original prerequisites |
| Whitespace and public/private provenance | PASS: changed-document whitespace/private-pattern checks and `git diff --check`; manual review confirms planning text only, no source-derived/private data |
| Existing progress freshness and synthetic tests | PASS: `python -B tools/progress.py --check` reports current; `python -B -m unittest discover -s tools -p 'test_progress.py'` runs all nine existing tests successfully |
| Cargo fmt/Clippy/tests/build and 180-frame smoke | NOT RUN: planning-only change; build/cache recovery and implementation remain next |
| Native Windows/Linux hardware, qualified-source readers/current features and controlled comparisons | NOT RUN here; missing proof remains required, not counted as completed support |
| New/modified tests, runtime, Cargo, coverage or source/save files | None |
| Task index regeneration | NOT RUN: prior PowerShell availability gap not re-probed; generated index not hand-edited |

An initial one-off documentation probe incorrectly expected the portable-save ticket to retain only its progress prerequisite; it correctly also retains its two already-resolved research/source prerequisites. Correcting that probe assumption passes; no tracker edge or existing test was changed. No new test/script file was created.

## Uncertainty, verdict and next work

**Decision/document checks PASS; implementation/source/runtime acceptance NOT RUN.** Unchanged-revision dependency recovery, native compilation, Wayland/launcher/audio work, real-reader source qualification, actual hardware, native Windows access and controlled comparisons still need evidence. No requirement gains coverage from this approval, and the complete first-stage matrix stays incomplete until its required cells/lanes are supported by proof. Permitted source-free build/startup work can begin before gameplay admission.

Next: recover the unchanged pinned dependency, capture baseline compiler diagnostics, then implement and verify the approved native Linux/source boundaries in evidence-sized tasks. The unmodified Windows-oriented code need not compile on Linux before porting starts; passing the final locked checks remains required. Keep the installed source and all existing saves untouched. Do not wait for the later progress/save decisions or claim full parity from an early no-save demo. The overall planning map remains open for those later decisions and observed fog. No commit, push, publication, package installation or new-test authorization is implied.

## Accepted checklist

Historical approval copied from the temporary tracker during T194 cleanup.
It remains an acceptance contract, not evidence that any check has passed.

**Accepted: the first no-save implementation handoff is ready.** Begin the main build/port work outside this planning map; do not wait for portable-save design. This is approval of the checks, not a result from running them, source admission or complete feature parity.

### Required platform/source cells

| Native platform and source | Required boundary for this stage |
|---|---|
| Windows × Steam | Preserve existing native Windows behavior, source checks, current features and existing persistence behavior; no implicit migration or save overwrite. |
| Linux × Steam | Verify the qualified Steam inputs through the native Wayland path and current readers/features. Clearly marked no-save operation is allowed in this first stage, not claimed as portable persistence. |
| Windows × selected Epic build | Qualify the selected Epic inputs independently and check current readers/features on native Windows. Early no-save operation is allowed; do not import vendor saves as a startup prerequisite. |
| Linux × selected Epic build | The first new host/source target: native Wayland plus independently qualified Epic inputs and current readers/features. Early no-save operation is allowed with an explicit warning. |

Retain all four cells and every required unverified feature/lane in the denominator. One passing cell does not pass another; a Linux result, cross-build, workflow declaration or Wine run is not native Windows evidence. A partial build/demo can be reported precisely as partial, but the complete first-stage matrix cannot pass while a required cell/lane is failed, blocked or unrun. These missing results do not prevent implementation from starting. Later save compatibility remains explicitly outside this first-stage finish line, not erased from the overall goal.

### Concrete checks

The expected results below follow the accepted source/platform contracts and preservation rules, not new values inferred from implementation. They describe manual/existing-check acceptance obligations; **they are not approval to create or modify tests or fixtures**.

| ID | Input or action | Expected result and evidence |
|---|---|---|
| A01 | Build the workspace with its pinned dependencies/native toolchain, then run the existing quality checks and source-free 180-frame smoke. | Locked checks succeed and the smoke completes normally. Preserve diagnostics for fetch, compile, test and runtime separately. A fetch, formatting check or timed window alone does not prove current features, audio or source qualification. |
| A02 | Open the native launcher, browse/select a folder, use existing rewrite helpers, and close them. | Wayland-native startup without Wine, readable required launcher text, usable native roots/paths, correct helper arguments/suffixes and orderly shutdown. Linux settings/cache/log locations follow the accepted XDG boundary; existing Windows locations/behavior remain intact. No vendor executable launch or installation write is required. |
| A03 | Request gameplay with an unknown, missing, changed, malformed or mixed required source set, including a remembered path or apparently matching cache. | Validate the currently selected source every gameplay session and refuse admission with a useful reason. Do not trust filenames/store metadata/cache, silently combine sources or bypass quarantine because saving is off. Negative probes must never damage the installed source. |
| A04 | Use a separately qualified source set with its actual selected language/resources/package order through the real Rust readers. | Required original/HD inputs, executable-derived inputs where needed, duplicate selections and asset/rule joins succeed for the current feature scope. Missing language/inputs or unresolved choices remain visible failures; no dropped feature, fabricated data or Steam substitute for Epic. E01 is not qualified merely by selecting it. |
| A05 | Switch source/profile/language/selection or reader version while derived data from another identity exists. | Revalidate selected inputs and reject mismatched/stale derived data. Never use a cross-source cache or config as a fallback, and never repair/update the installation. Any regeneration belongs in private external output storage. |
| A06 | Run the current graphics, audio and input features on the selected Linux hardware. | Record actual Wayland backend/GPU/driver and presentation, audible output/device/format/rate, and keyboard/mouse behavior. Check focus/scaling/shutdown, audio stream lifecycle and applicable controller mappings/hotplug separately. A private software-rendered window or virtual sink is functional evidence only, not real GPU/speaker/controller proof. Untested required devices remain open. |
| A07 | Repeat applicable existing current-feature scenarios on native Windows and each qualified source, including title/startup, menus/HUD/camera, current movement/combat/scripts, animation/effects and sound. | Preserve the implemented feature scope and Windows/Steam behavior. Distinguish existing finite scripted regression checks from controlled installed/rewrite comparisons. Do not invent game rules or import Steam-derived rule assumptions into Epic without evidence. |
| A08 | Start an early no-save run with qualified required source inputs but no vendor save/account, and attempt the unavailable persistence actions. | The no-save path does not depend on the rejected executable cipher or vendor account/save. Clearly warn that progress will not be saved; saving/loading/import stay unavailable rather than report success, seed blank progress or overwrite files. Source admission still applies. Existing rewrite/vendor saves and existing Steam persistence behavior remain intact. |
| A09 | Perform a controlled installed/rewrite comparison for a source-bound current-feature scenario. | Record common initial conditions and timestamped inputs; exact record/discrete-state agreements and independently measured numeric/timing/presentation comparisons follow the fidelity contract. Missing reference settings, devices, measurements or tolerances remain BLOCKED/NOT RUN; a screenshot, matching archive count or generated-arena tour is not 1:1 proof. |
| A10 | Required Windows access, physical hardware, qualified inputs or reference evidence is unavailable. | Keep the affected cell/feature visibly BLOCKED/NOT RUN with its missing prerequisite and next action. Continue permitted build/port work without claiming that lane passed, removing it from coverage or broadening support to untested controllers, distributions, X11 or Deck. |

### Existing checks stay intact

Put the Cargo bin directory on PATH and retain the pinned Rust/Bevy/revision baseline. Recover the unchanged dependency non-destructively as agreed; do not change revisions or upgrade unrelated dependencies to obtain a passing result.

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo run -p main --locked -- --frames 180
python -B tools/progress.py --check
python -B -m unittest discover -s tools -p 'test_progress.py'
```

Existing validator/source-mutation, controller and synthetic save checks remain unchanged; their narrow results do not qualify hardware or portable saves. Never delete, skip, weaken or rewrite an existing test to hide a failure. Report an intentional behavior conflict before proposing changes. If a new test is needed, present concrete spec-derived input → expected-output examples and obtain separate explicit approval before writing it. No remote CI dispatch, push, package installation, global configuration change or release is implied.

### Evidence and handoff

Use a task record per nontrivial implementation slice, tied to the rewrite commit/dirty changes, host, exact commands, relevant platform/source cell, opaque source/profile identity, feature scope and evidence. Record PASS/FAIL/BLOCKED/NOT RUN per lane and first divergence. Exact source hashes/paths, extracted bytes, executable details, logs, captures and saves stay in private external storage; public records use opaque IDs and safe pointers. Keep actual source checks distinct from historical inventories. Do not modify the installed tree or existing saves to manufacture a rejection/preservation scenario; use existing safe checks, and seek approval if additional tests/fixtures are needed.

Linux GUI work uses a private `kwin-mcp` session, never the user's desktop, with session cleanup on every exit. Record software rendering honestly; it cannot satisfy hardware requirements. Native Windows execution, physical listening/controller review and controlled reference comparisons remain their own evidence/access lanes.

No new reference display configuration, controller choice, simulation clock or numerical tolerance is selected here; the fidelity contract's measured-evidence rules and remaining final-gate questions remain unchanged. Update reviewed coverage only when implementation evidence actually qualifies units, retaining all required unknown/failed units. This decision alone changes no coverage or runtime/source gate.
