# T175 — Choose the first native Linux target

Owner: coding agent with the project owner. Gates: P00/P03/P30 planning only; no source/runtime/fidelity gate advances. Prerequisites: fidelity contract, milestones, unknowns, folder guide, glossary, T174, Wayfinder/grilling/domain-modeling, compatibility map, native host and reproducible-build research decisions.

## Decision and identity

After the source decision, the user asked to move on to the Linux build/startup decision. They accepted **Yes, use this plan (Recommended)** after being shown: recover the existing pinned build without upgrade shortcuts, target the current Wayland desktop first with X11 later, add Linux sound without replacing the mixer, use a freely licensed launcher font and normal Linux folders/native helpers, preserve Windows, and verify graphics/sound/input separately. Saving remains off for early runs under the previously accepted boundary; game rules do not change for a platform port.

This resolves **Choose the native Linux platform contract**. It is one HITL decision, not a build or a port implementation. The user accepted the boundary and work order, not a particular audio-library/font version, numerical fidelity tolerance or proposed new test. Research-based implementation starting points are identified as such in the answer, not falsely attributed to a user choice.

Rewrite identity verified with `git rev-parse HEAD` and `git branch --show-current`: `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f`, `epic-linux`. Rust 1.99.0, Bevy 0.19.1 and the existing lock/revision remain unchanged. First Linux scope: the previously inspected CachyOS x86_64/Wayland development host. Prior installed-library/device inventory is not re-measured here and does not prove hardware operation. E01 remains unqualified. No source installation, executable, source-derived output, save, application or GUI was accessed or changed.

## Steps and exact evidence locations

1. Claim **`.scratch/native_linux_epic/issues/06-native_platform_contract.md`** with the project-owner assignee before investigation. Load the map and required project records; consult the interview/domain-modeling skills and established glossary distinctions. Do not mix source qualification, platform compatibility, feature parity or the two save types.
2. Review **`docs/research/linux_host_boundaries.md`** and **`docs/research/build_verification_baseline.md`**. These contain the pinned-primary-source links and prior bounded host/cache observations: missing Linux window features, Windows-only output, launcher font/path/helper assumptions, available but unproven host libraries, upstream dependency reachability and incomplete local cache. No new API research, remote availability probe or native execution is claimed.
3. Present the focused plan in plain English and obtain the live answer. The X11-now alternative was not selected. Retain the prior source/no-save safety decisions; no extra questionnaire about save formats or game rules is needed.
4. Append `## Discussion` and `## Answer` in **`.scratch/native_linux_epic/issues/06-native_platform_contract.md`**, then mark it resolved. The answer records explicit Wayland features without enabling all Bevy defaults, native GNU/Windows MSVC preservation, non-destructive unchanged-revision cache recovery, the Linux-only device adapter/mixer seam, licensed launcher-font coverage obligations, XDG categories, native roots/helper paths and separate evidence lanes. CPAL =0.16.0 and Liberation Fonts 2.1.5 are reviewed starting points, not proof of audio/glyph correctness or new installed artifacts.
5. Keep normal Linux folders separate from save-format approval and source quarantine. Existing Windows locations remain unchanged; no automatic migration. Private/source-derived outputs stay outside the checkout. Source-free startup checks do not qualify a source or authorize gameplay. No-save runs must warn truthfully and preserve existing progress/vendor files.
6. Add the named resolution to **`.scratch/native_linux_epic/map.md`**. No new sharp decision emerged; existing hardware/driver fog remains. **Approve the compatibility acceptance matrix** is now unblocked for the first no-save handoff. Portable progress/save tickets stay open for later work; the map itself remains open.
7. Update only the accepted first-host architecture and staged-platform summary in **`docs/fidelity-contract.md`**. Leave historical source evidence, the broad unmeasured hardware floor, comparison rules and final gates intact. Existing glossary terms suffice; no glossary, coverage ledger, report, code, manifest, lock or test change is needed.
8. Check changed-document links/whitespace/private patterns, tracker state/index/dependency consistency and the existing progress freshness/synthetic suite. Do not launch a game, use the user's desktop, install packages, change global configuration or weaken tests. GUI/hardware probes remain downstream and require private sessions with external evidence retention.

Exact decision evidence: the local ticket's `## Discussion` and `## Answer`, its map entry, and this record. Technical evidence remains the two research reports above and their pinned primary-source references. No private fingerprints, bytes, captures, addresses, hardware serials or personal paths are introduced. No new private evidence directory is created.

## Checks

| Check | Result |
|---|---|
| Live focused-plan approval | PASS: explicit selection; X11-now alternative not selected |
| Rewrite build/source identity | PASS for unchanged rewrite HEAD/branch only; source admission NOT RUN |
| Resolution, index and dependency graph | PASS: nine children, eleven acyclic edges, six resolutions; named decision indexed; first-stage acceptance is the next unblocked child; both later save questions remain open |
| Local links, whitespace and public/private boundaries | PASS: four changed documents checked; local links resolve; no trailing whitespace or private-path/hash/address patterns; `git diff --check` passes; provenance review confirms planning text only |
| Existing progress freshness and synthetic tests | PASS: `python -B tools/progress.py --check` reports current; `python -B -m unittest discover -s tools -p 'test_progress.py'` runs all nine existing tests successfully |
| Cargo fmt/Clippy/tests/build and 180-frame smoke | NOT RUN: documentation-only decision; dependency recovery and compilation remain downstream |
| Native Linux/Windows graphics, audible output, devices and source comparisons | NOT RUN; inventory is not runtime proof |
| New/modified tests, runtime, Cargo, coverage or source/save files | None |
| Task index regeneration | NOT RUN: prior PowerShell availability gap not re-probed; generated index not hand-edited |

## Uncertainty, verdict and next task

**PASS for the confirmed human platform decision only; technical acceptance NOT RUN.** The existing pinned build still needs cache recovery and compilation. Hardware renderer/presentation, audio negotiation/callback behavior, launcher font coverage/layout, real input devices and native Windows regression evidence remain unverified. New dependency/library choices may only be justified by actual requirements, not silent upgrades. Missing access or device evidence remains BLOCKED/NOT RUN; no hardware or final gate gains coverage from this decision.

Next: **Approve the compatibility acceptance matrix**, the short final evidence/checklist decision for the first no-save implementation handoff. Then implement and verify the port; do not keep the main work blocked on portable-save formats. Portable progress/save choices remain later work. No commit, push, publication or new-test authorization is implied by this approval.
