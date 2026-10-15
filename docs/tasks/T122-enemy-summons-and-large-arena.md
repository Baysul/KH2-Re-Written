# T122 — Source-bound enemy summons and expanded animation test arena

Owner Codex, started 2026-10-13, continued 2026-10-14. Gates P10/P12/P13 development inspection, supporting
P15/P19/P21/P27; no final gameplay gate passes. User request: use REA and Ghidra
to implement every Heartless/Nobody, their spawn/attack/idle animations, and a
larger room with summon controls. Prerequisites: fidelity contract, milestones,
unknowns, T106 enemy inventory/script decoder and latest T120/T121 work.
Build identity: HEAD `3aa251e`, package 0.1.1, preserving all pre-existing
uncommitted changes. Source identity: Steam app 2552430/build 15194255 English
candidate; complete supported edition/dependency profile remains unqualified.

## Steps and exact evidence

Private runtime evidence root:
`%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-room-20261013/`.
Private native evidence root:
`%LOCALAPPDATA%/KH2FinalRewrite/research/enemy-native-20261013/`.

1. Read required records, existing Shadow runtime and T106 decoder. Use the
   REA skill's independent investigation workflow for a native evidence pass,
   asset catalog and presentation module. Existing unrelated changes remain
   uncommitted. Installed files and saves are read-only throughout.
2. REA 6.4.0 canonical managed inspection binds the actual mixed CLR/native
   executable (`rea-managed-01.json`). Its native provider remains unsupported
   for this target. Direct Ghidra uses the existing source-matching project,
   read-only, without analysis, and requires exact instruction/source-byte
   agreement; retain the native receipts and pseudocode outside Git.
3. Build the roster from installed object entries and Jiminy character records.
   Join explicit journal ObjectId, not the distinct gameplay character field.
   Exact model reuse is recorded as inference; unresolved candidates and
   missing assets remain visible. No filename prefix proves species or complete
   encounter reachability. Retain bosses and variants as candidates rather than
   silently excluding required enemies.
4. Inspect source script action registration and literal motion requests through
   direct callback graphs. Preserve conditional choices, dynamic requests,
   decoder failures and unknown callbacks. Native MSET header bit 0 selects
   logical-motion times four plus a context-dependent variant; expose every
   populated candidate variant instead of inventing the actor's context.
5. Add on-demand source model/motion loading, independent animation actors and
   F3 summon/category/actor/action/motion pages. Freeze, step, restart, loop,
   rate and root-translation controls operate independently. Clear/remove
   releases meshes, materials and textures; area changes clear preview actors.
   Linked model blocks not presented by the current renderer stay explicitly
   reported. These actors have no generic replacement AI or combat rules.
6. Expand generated test arenas to at least 9000 by 9000 scene units, with
   800-unit walls and bounded tile density. Apply this development choice in
   memory to existing configs too. Fix the tiled floor boundary for dimensions
   not divisible by tile size; visual and collision surfaces now end at walls.
   The arena is authored development geometry, not an original game room.
7. `corpus-01.json` retains the first scan: 304 candidate records, 274 model
   loads, 6,440 successful motion samples and 461 motion failures. Inspect actual
   failing source layouts in `failed-model-layouts-01.json` and retain original
   spans privately in `failed-model-originals-01/`. Two failures were caused by
   unrequested HD textures. Add an explicit original-only reader while keeping
   the existing readers unchanged. Eleven custom/disabled material profiles and
   seven mesh-bearing missing-material archives now use an explicitly marked
   geometry-only inspection; no replacement textures are fabricated.
8. Decode type-one raw matrices with an explicit discrete stored-matrix profile.
   Independent private `raw-oracle-02.json` under the native evidence root
   compares 69 motions, 209,922 animation matrices, 794 position matrices and
   3,371,456 complete words. Source records with nonzero starts revealed an
   indexing error; subtract the declared start before indexing. Generated
   regressions cover it. Native interpolation, cadence and root semantics remain
   unqualified. The local-pose/IK probes reject raw input instead of indexing
   absent interpolated headers or synthesizing channels.
9. `corpus-final.json` retains 42 failed motion inspections. Twenty-seven have
   negative declared start frames; preserve and evaluate those signed source
   keys in the evaluated profile. The legacy unsigned basic FK profile rejects
   them explicitly. `corpus-final-02.json`, with complete fresh source scans
   before and after the audit, retains 304 candidates: 82 journal/model-classed
   Heartless, 11 Nobodies and 211 unclassified candidates. Of these, 294 primary
   models load, including 276 original-indexed textured inspections and 18
   geometry-only material warnings. Ten remain failed: nine archives have no
   model BAR entry, and one has an unsupported model range. Every row remains
   present. Across loaded models, 7,323 of 7,338 populated motion slots pass
   start/middle/end finite pose sampling; 15 raw timeline mappings remain FAIL.
   This is a finite inspection denominator, not a complete reachable roster or
   full-frame animation/reference equivalence.
10. Static source inspection retains 8,187 named actions and 4,958 literal
    motion-request candidates. Of the actions, 2,528 have callback errors or
    dynamic motion requests. Conditional choices and unknown actor variants
    remain visible. An automatic appearance preview runs only for an
    unambiguous supported direct source request; an optional equally
    unambiguous idle follows under explicit gallery playback policy. This does
    not execute the original scheduler, effects or combat traps.
11. `tour-01/` checks Shadow and Dusk. `tour-final/` adds the raw-motion Mushroom
    IV. `tour-delivered/` repeats through the actual updated release launcher:
    three independent actors, six source-slot changes, seven nonuniform captures,
    complete tour verdict PASS and exit 0. Inspect its captures 00 and 06 for
    source characters/large arena and the actual summoned-actor list. This is a
    renderer/integration tour, not a physical input or paired reference test.
12. Preserve both prior executables with verified hashes under
    `%LOCALAPPDATA%/KH2FinalRewrite/backups/T122-launcher-20261014/`. Build with the
    existing `skate3` release feature and update `target/release/KH2RW Launcher.exe`;
    `launcher-receipt.json` binds the delivered and built executable identities.
    No installed file, save, configuration or pre-existing launch selection is
    overwritten. No commit, push or release publication.

## Checks

- PASS fresh canonical REA identity and finite source-byte-matched Ghidra
  research; these do not qualify complete logical function coverage.
- PASS initial integrated compile (`check-03.log`).
- PASS `cargo fmt --all --check` (`fmt-final-02.log`).
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`
  (`clippy-final-02.log`).
- PASS `cargo test --workspace --locked`: 513 passing executions
  (`tests-final-02.log`), including generated script, raw data, signed-window,
  independent playback, failure preservation and GPU teardown controls.
- PASS `cargo build --workspace --locked` (`build-final-03.log`).
- PASS `cargo run -p windows_shell --locked -- --frames 180`
  (`smoke-final-03.log`); synthetic window, default controller mapping warning.
- PASS final release build (`release-build-final.log`), preserved backups,
  delivered executable identity and actual release-renderer tour.
- PASS complete audit execution/source validation (`corpus-final-02` logs).
  Individual finite model/motion checks retain the 10/15 FAIL records above.
- PASS generator and nine synthetic progress tests; required comparison units
  remain NOT RUN, inventory incomplete and final roadmap acceptance unchanged.
- PASS whitespace/changed-code/provenance review; new repository content is
  authored Rust/Markdown and public progress metadata. Private source, arrays,
  disassembly, reports, captures and executable backups remain external.
- Early diagnostic/build errors are retained. `build-final-02.log` failed
  because Windows locked the running audit executable; after its normal exit,
  `build-final-03.log` passes. No source changes were required for that retry.
- NOT RUN installed/rewrite enemy timing, AI, damage, spawn and rendering
  comparisons. Complete enemy behavior remains required.

## Uncertainty, verdict and next task

Verdict: PARTIAL. The larger arena and summon/animation
inspection implementation do not complete the requested enemy gameplay.
Complete enemy scheduler/trap execution, encounter activation, all multi-actor
target/hit identities, contextual animation selection, motion effects, linked
model presentation and installed comparisons remain open. No final gate or
whole-game coverage claim advances from compiling or displaying source assets.

Next: retain and resolve corpus failures, qualify the action/callback/trap
denominator, and translate/compare complete enemy behavior in source-derived
encounters. Source profile qualification and controlled reference comparisons
remain mandatory before gameplay acceptance.
