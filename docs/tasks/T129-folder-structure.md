# T129 — Simple folders and names for contributors

Owner Codex, 2026-10-15. Gate P04 repository organization only. Prerequisites:
fidelity contract, milestones, unknowns, T128, existing architecture and the user's
request for descriptive names and main folders such as game, visual, effect and
main. Build identity: HEAD `3aa251e`, package 0.1.1, pinned Rust 1.99.0 Windows
MSVC; pre-existing uncommitted work retained. Source identity: no installed-source
read or gameplay admission. Existing validation remains required per session.

Private evidence: `%LOCALAPPDATA%/KH2FinalRewrite/logs/T129-folder-structure/`.
Pre-edit maintained-file backup:
`%LOCALAPPDATA%/KH2FinalRewrite/backups/T129-folder-structure/`.

## Steps and evidence

1. Read the required contracts and latest record, manifests, callers, asset
   references, build scripts, documentation and default-deny allowlist. Preserve
   the 919 maintained files present before restructuring, including previously
   untracked work. `inventory.json` and `git-before.txt` in the backup identify
   that starting state. Private local settings, data, saves and retained release
   programs are outside the moved source folders.
2. Replace the generic `crates/` collection with `game/player`, `game/area`,
   `game/collision`, `game/event`, `effect`, `audio`, `data` and `source`. Split
   the former presentation/application package into shared `visual` and program
   wiring in `main`; move the launcher to `main/src/main.rs` and developer
   programs to `main/src/bin`. Keep the authored smoke image with its program;
   keep the banner/logo with presentation. Preserve the existing optional
   skateboard code, attribution and GPL boundary under `extras/skateboard`.
3. Rename owned Cargo packages and display modules to match their purpose.
   Examples: `player_sim` becomes `player`, `camp` becomes `pause_menu`, `pad`
   becomes `controller`, `bobs` becomes `scenery`, and `ui2d` becomes `overlay`.
   Explicit external-crate paths resolve collisions with the scene `area` module
   and effect-file reader. The first Clippy attempt caught these and one overly
   broad upstream import rename; restore upstream Rust source bytes and correct
   the imports before rerunning. Retain `clippy-initial.log` honestly.
4. Update Cargo paths/lockfile, Rust imports, embedded shader references, asset
   references, build helper, source-scanner helper, ignore rules and current
   guides together. No new external dependency or dependency-version change.
   `moves.json`, `changed.json`, `source.diff`, `preservation.json` and `links.json`
   are the private move map, rename-aware review and preservation/link evidence.
5. Add `docs/folder-guide.md`, short READMEs in the main folders, and future
   naming/placement rules in AGENTS.md and CONTRIBUTING.md. Rewrite the stale
   architecture map. Records through T128 retain their original evidence paths
   and commands; the generated task index links the current name map. The older
   reorganization proposal is explicitly historical. No empty future subsystems,
   mod-loading API or stable mod package format is introduced.

## Checks and limits

PASS initial preservation and navigation audit: all 919 original maintained files
exist at their original or mapped destination; 777 retain identical bytes at that
checkpoint. All 429 external lockfile records retain their complete original
versions, sources, checksums and dependency lists. No new broken Markdown file
links were introduced. Binary assets and licenses are preserved, not reauthored.
Final verification results are recorded below when complete.

NOT RUN installed gameplay or controlled installed/rewrite comparison. Existing
unfinished menu/title state in presentation, scene assembly in the application,
and enemy controllers in the player package remain documented limitations.
Source qualification, game rules and public coverage acceptance do not change.
The source-bound cache remains planned. Historical dependency inventory describes
its original audit; it is not silently rewritten as a new dependency audit.

Verdict: NOT RUN final engineering checks while verification is in progress.
Next: finish required checks and the final provenance/ignore review, then use the
folder guide as the placement convention for subsequent feature work.
