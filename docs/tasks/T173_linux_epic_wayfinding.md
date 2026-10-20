# T173 — Chart native Linux and Epic compatibility

Owner: coding agent on behalf of the project owner. Gates: P00/P02/P03/P06 and platform/verification planning only; no acceptance gate advances. Prerequisites: required scope/roadmap/unknown/layout records, T170, historical T171/T172 evidence, Wayfinder/local Markdown tracker, grilling, domain modeling, and the user's live scope answers.

## Destination and identity

The user requested actual Epic compatibility and native Linux support in addition to Windows. This session charts the decision route under Wayfinder's planning-only default; it does not deliver or qualify the port. Confirmed scope is current-feature parity across Windows/Linux × Steam/Epic; the first Linux target is the inspected x86_64 CachyOS/Wayland host. Delivery is reproducible source builds plus the native launcher. Epic means qualified local files, not online services. New rewrite saves must be portable across qualified platform/source combinations where game-data compatibility is established, rejecting incompatible versions; Epic vendor-save import is deferred.

Rewrite base: `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`, branch `epic-linux`; Rust pin 1.99.0, Bevy pin 0.19.1. Source candidate E01 is only the historical private T172 observation, still quarantined/unqualified. No new source qualification, gameplay session or vendor-save access is part of charting. The existing fidelity contract remains unchanged until a later accepted decision explicitly amends its platform/source scope.

## Steps and exact artifact locations

1. Read the supplied Wayfinder skill and its grilling/domain-modeling dependencies, required project docs and latest current-worktree task. Confirm the clean `epic-linux` worktree lacks a configured tracker or glossary; choose the documented local Markdown fallback rather than create a remote tracker.
2. Obtain six live scope answers: current-feature parity, this Linux host first, rewrite saves before Epic vendor-save import, local Epic files without store services, builds/native launcher before packaging/publication, and validated cross-platform/source portability of new rewrite saves. Separate source compatibility, platform compatibility and vendor-save import in `GLOSSARY.md`.
3. Bring forward the four existing historical files byte-for-byte from the original worktree: `docs/research/linux_epic_source_assessment.md`, `docs/tasks/T171_linux_epic_feasibility.md`, `docs/research/epic_reader_boundaries.md`, `docs/tasks/T172_epic_source_probe.md`. Do not change the original worktree or retroactively alter those observations.
4. Create canonical map **`.scratch/native_linux_epic/map.md`** and eight separate child issue files under **`.scratch/native_linux_epic/issues/`**, using the local tracker's numbered-file convention with descriptive snake_case slugs. Create every child before a second pass wires real blocking references. Keep the map an index, not a duplicate ticket store; do not list open children there.
5. Claim the four AFK research children before delegation, leaving the four HITL policy/acceptance children open and blocked. Research runs in isolated local `research/<name>` branches. Privacy overrides the generic skill's push step: no remote pushes, PRs, merged research branches or private research publication. Review actual report artifacts even if a transport status is failed.
6. Research report destinations: **`docs/research/linux_host_boundaries.md`**, **`docs/research/epic_source_inputs.md`**, **`docs/research/portable_rewrite_saves.md`**, **`docs/research/build_verification_baseline.md`**. Main read every report, verified each isolated branch/base/common-directory identity, and copied the four public-safe artifacts byte-for-byte without merging branches. Each belongs to its named child; appended answers link the report rather than paste it into the map. Unknown evidence remains explicit; recommendations do not settle HITL choices. The blank progress capture and discarded load payload were directly traced in `source/main/src/save_files.rs:103-105` and `source/main/src/title.rs:549-554`; interactive startup's executable-table dependency was traced in `source/main/src/save_files.rs:46-69` and `source/main/src/title.rs:844-864`.
7. Resolve only the four AFK fact-finding children. Their answers expose a precise new HITL decision, **Define the portable progress capture and restore boundary**, at `.scratch/native_linux_epic/issues/09-progress_capture_restore.md`. Create it first, then wire its real blockers and make the portable-save contract depend on it; the refreshed graph has nine children and twelve edges. Narrow the remaining fog to unobserved feature/resource and device/driver failures rather than duplicate live questions.
8. Add narrowly scoped tracking rules only for `GLOSSARY.md`, this map and its child Markdown files; explicitly ignore generated `.pi/fabric/` metadata/research worktrees. Keep other scratch data default-denied. Link the planning entry point from `AGENTS.md` and `docs/folder-guide.md`; do not scaffold runtime systems or amend Cargo/runtime/tests/coverage.
9. Verify local links, graph/claims/types/labels/frontier, privacy/provenance, ignore boundaries, source preservation and relevant existing report checks. Record results below. Do not add tests or leave scratch scripts/fixtures; new behavioral tests require explicit approval of concrete input → expected-output examples in the acceptance decision.

## Checks

| Check | Result |
|---|---|
| Destination/scope live confirmation | PASS, six explicit user answers |
| Initial map and eight child files; second-pass dependencies | PASS: eight typed/labeled children, nine acyclic edges, four AFK claims before delegation |
| AFK research artifacts and resolutions | PASS: four actual artifacts reviewed, provenance checked, copied unchanged and linked from four resolved research answers; runner transport status remains FAIL |
| Refreshed graph/frontier | PASS: nine children, twelve acyclic edges, four resolved research and five open HITL decisions; source qualification and native platform contracts form the frontier |
| HITL decisions resolved in charting | None; they remain open for live human exchange |
| Links/privacy/provenance/ignore boundaries/whitespace | PASS: all 23 changes are public planning/docs; named index/local links valid; eight copied reports/records unchanged; narrow public/private ignore probes and `git diff --check` pass |
| Existing progress freshness and synthetic tests | PASS: `python tools/progress.py --check` current; all nine existing `tools/test_progress.py` tests pass |
| Native Linux/Windows build, GUI, audio/input and source qualification | NOT RUN by charting; any bounded research probe must be distinguished from acceptance |
| New or modified tests | None; examples/approval still required before future additions |
| Runtime/Cargo/coverage/fidelity contract changes | None |
| Task index regeneration | NOT RUN: PowerShell unavailable; generated task index not hand-edited |

## Research execution limits

All four delegated runners returned **FAIL** with `Child Pi settled; child did not exit after stdin closed for 5000ms`. All four reports existed as their only changed files on the required local research branches at the exact baseline. Main verified and reviewed those actual artifacts; the research-answer PASS records accepted bounded fact-finding, not a successful runner exit or runtime check. Research branches remain local, unmerged and unpushed. One worker also logged an unrelated history-extension rename error; no project-file correction was needed.

Workers lacked Context7 tools and used pinned primary sources. Main then queried Context7 IDs `/websites/rs_bevy` and `/rustaudio/cpal`; returned material was `latest`/`master`, including later CPAL APIs, not proof for Bevy 0.19.1 or candidate CPAL 0.16.0. It was not substituted for the reports' pinned primary evidence or treated as an accepted backend choice.

The build investigation found the unchanged dependency revision reachable upstream while the local Cargo cache remains incomplete. No cache repair, locked build, native runtime, source admission or Windows verification was performed. The save investigation found capture/apply is absent, not a working progress snapshot; defining those semantics is now a separate human decision before format choices. E01 remains unqualified, and none of these results establishes cross-source game-data equivalence.

## Uncertainty, verdict and next step

**PASS for charting and bounded research review only; compatibility implementation and native acceptance NOT RUN. No game-fidelity gate PASS claimed.** The four research questions have bounded answers; five human decisions remain open, including the newly exposed progress boundary. The canonical map remains open until those decisions and genuine remaining fog are resolved. Prior finite archive-format observations and the real save-table reader rejection remain historical T172 evidence, not new runtime results.

Next Wayfinder session: take the first unblocked/unclaimed child by tracker order: **Choose the source qualification and profile contract**. **Choose the native Linux platform contract** is independently takeable. Claim a chosen ticket before work and resolve at most one HITL decision per later session, with the user. Do not start implementation merely because the chart exists. The requested port remains an open downstream objective.
