# T194 — Clean up Linux/Epic development artifacts and merge local main

Owner: coding agent. Gate: development integration only; no source, platform,
gameplay or fidelity gate advances. Prerequisites: T177–T181 and T193. The user
explicitly requested cleanup, untracking the glossary, and resolving any
conflicts before merging with the latest remote main. Local integration is
authorized; no push or publication is performed.

## Build and source identity

Fetched `origin/main`: `3645961`. The paused rebase replayed `3a3f5c3` onto that
base, producing the cleaned port commit `b597f0d`. Rust/Cargo 1.99.0, Bevy
0.19.1 and the existing dependency pins are preserved. Verification uses code
and existing synthetic checks only; no installed game source or save is read
or changed. The user's launcher configuration and build outputs are retained.

## Evidence and steps

Private bundle **T194-B01**, outside the checkout: `before-cleanup.patch`,
`status-before.txt`, `upstream-main.txt`, `removed-duplicates.txt`, retired
tracker/probe/Fabric metadata, recovery copies of duplicate reports,
`rebased-head.txt`, `status-after-rebase.txt`, `fetch-final.log`,
`check-results.txt`, `check-*.log`, `test-summary.json` and
`final-verification.json`. Existing private evidence remains outside Git.

1. Remove the ten temporary `.scratch/native_linux_epic` Markdown files and the
   `output_check` audio example. Remove their Git allowances and current probe
   instructions. Preserve the full accepted checklist in T176; point current
   guides and prerequisites at the durable T174–T176 records. Historical task
   descriptions retain the original path and command spellings.
2. Untrack `GLOSSARY.md`, keep the local copy in the original worktree and
   explicitly ignore it. Remove current tracked links to that local-only file.
3. Remove four temporary research worktrees and their already-merged research
   branches. Before removal, verify their untracked reports are byte-identical
   to the maintained copies. Similarly remove four byte-identical duplicate
   reports from the target main worktree, with private recovery copies.
4. Remove generated Fabric worktree metadata and Python bytecode from the
   checkout. Keep the maintained research reports, runtime readers/devices,
   licensed fonts, game settings and existing build directories.
5. Complete the paused rebase with the cleanup included. No conflict markers or
   unmerged entries remain. Merge `epic-linux` into local `main` with Worktrunk,
   without additional commit/squash/rebase, hooks, worktree removal or push.
6. Fetch `origin/main` again after the merge. It is unchanged and is an ancestor
   of local main. Both branches contain `b597f0d`; the merge is a fast-forward,
   not a redundant merge commit. Run the existing checks on this merged tree.

## Checks

| Check | Result |
| --- | --- |
| Latest fetched upstream main included in local main | PASS |
| Rebase finished and local merge completed | PASS: `b597f0d` |
| Scratch tracker/audio probe absent; glossary local, ignored and untracked | PASS |
| Temporary research worktrees removed; duplicate reports preserved in maintained copies | PASS |
| Current links to retired scratch/glossary and probe instructions removed | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo build --workspace --locked` | PASS |
| `cargo test --workspace --locked --no-fail-fast` | FAIL: 895 passed, 2 failed, 1 ignored, 30 test programs |
| Progress freshness and existing progress-tool tests | PASS: 9 tests |
| New or changed tests | None |
| Installed-source gameplay, GUI/audio and native Windows checks | NOT RUN: no runtime behavior changed by cleanup |

## Uncertainty, verdict and next work

**Cleanup and local integration PASS; complete test verification FAIL.** The same
two enemy weapon-trap tests reported in T193 fail with `the trap is translated`
at `source/game/enemy/src/traps/weapon/tests.rs:34:36`. The entire enemy package
is unchanged from upstream. It is not patched, skipped or weakened to make the
merge look green. The user requested completing integration despite these
separate reported failures. No overall fidelity or compatibility gate passes.

Next: investigate the two upstream enemy-script test failures separately. The
local merge is complete; publishing remains a separate user decision.
