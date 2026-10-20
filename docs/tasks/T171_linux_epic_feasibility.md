# T171 — Native Linux and Epic source feasibility

Owner: coding agent. Gates: P00/P02/P03/P06/P08/P23/P29/P30/P31 planning only; no final gate advances. Prerequisites: `docs/fidelity-contract.md`, `docs/milestones.md`, `docs/unknowns.md`, `docs/folder-guide.md` and T170. Request: assess what native Linux execution and Epic Games Store source support would require, not implement either or silently replace the Steam baseline.

## Build and source identity

Audited rewrite commit: `554d3bf18d3b`; Linux x86_64; Rust/Cargo 1.99.0; Bevy pinned to 0.19.1. The historical selected Steam candidate remains unqualified; this audit did not revalidate or launch it. No Epic installation was supplied or qualified. No game data, source hashes, executable extracts, saves or captures were produced or added to the checkout.

## Steps and exact evidence locations

1. Read the scope, roadmap, unknowns, layout guide and latest task; trace source admission, data reading and native process launches. Separate host portability from source-edition qualification.
2. Trace the concrete platform boundaries: `source/main/Cargo.toml:30`, `source/visual/Cargo.toml:20`, `source/audio/src/lib.rs:247-277`, `source/main/src/launcher/paint.rs:103-135`, `source/main/src/launcher.rs:299-309`, `source/main/src/launcher.rs:400-431`, `source/main/src/launcher/install.rs:24-39` and `source/main/src/launcher/install.rs:601-609`.
3. Trace candidate identity/session checks and save coupling: `validation/src/lib.rs:213-329`, `source/data/src/probe_source.rs:142-285`, `source/game/save/src/store.rs:60-133` and `source/main/src/save_files.rs:43-70`. A matching configuration or Steam manifest does not qualify Epic files. Reading a PE executable for tables does not require running it.
4. Check pinned primary feature metadata: [Bevy 0.19.1 Cargo manifest](https://docs.rs/crate/bevy/0.19.1/source/Cargo.toml.orig). This identifies window-backend features, not application runtime compatibility.
5. Attempt `cargo check -p main --bin KH2RW --locked`; dependency retrieval times out before compilation. The bounded repeat `cargo check -p main --bin KH2RW --locked --offline` exits 101: the existing workspace's `skate-data` Git dependency cannot be checked out offline. No manifest or lockfile changes bypass the dependency.
6. Record reuse candidates, concrete work packages, save/account risks, uncertainty and a shortest route in `docs/research/linux_epic_source_assessment.md`. The independent source/save audit was incorporated; its agent transport reported failure after writing the report, so the file was read and checked directly rather than treating process completion as verification.

Evidence is the public source locations, pinned upstream manifest and assessment above. There is no private installed-source evidence for this task; no Epic compatibility claim is made.

## Checks and uncertainty

| Check | Result |
|---|---|
| Static audit identifies platform and storefront-specific boundaries | PASS, bounded code inspection only |
| Native Linux compilation | BLOCKED at dependency retrieval, before crate compilation |
| Native Linux graphics/audio/input/save/load smoke | NOT RUN |
| Epic inventory, package decoding and executable-derived table compatibility | NOT RUN; actual selected files required |
| Steam/Epic profile/cache isolation and unknown-build behavioral checks | NOT RUN |
| Existing workspace tests, Clippy, build and frame smoke | NOT RUN after the dependency blocker; no runtime implementation changed |
| Tests added or modified | None |
| Cargo lockfile | Unchanged |
| Task index regeneration | NOT RUN: `pwsh` is unavailable on this host; the generated index was not edited by hand |
| Coverage/report ledger | Unchanged: no decompilation unit or controlled installed/rewrite comparison was qualified |

Uncertainty: Linux runtime behavior and Epic's relevant package/executable/save differences remain unobserved. The Linux host work has identifiable boundaries, but no calendar estimate or compatibility guarantee follows from static inspection. Epic save import is separate from making native rewrite persistence work. Neither platform support nor a new source profile establishes whole-game equivalence.

Verdict: Feasibility assessment recorded; Linux build BLOCKED and Linux/Epic runtime compatibility NOT RUN. No final gate passes, supported-source profile is added, or product scope is silently changed. Installed files remain untouched; no purchase, edition switch, publication or vendor-save write occurred.

Next: if implementation is requested, explicitly extend the Windows/Steam contract, establish a Linux environment smoke without weakening source admission, and privately qualify a user-selected Epic installation before enabling it for gameplay. Regenerate the task index with `tools/New-TaskIndex.ps1` when PowerShell is available.
