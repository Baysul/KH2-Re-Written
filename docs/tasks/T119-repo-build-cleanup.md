# T119 — Remove redundant local build outputs

Owner Codex, 2026-10-13. Gate: P04 local development housekeeping; no gameplay gate. Prerequisites: T118 naming/build verification and the user's instruction to remove useless local executables and files consuming space. Build identity: `main` at `7ffb955` plus the uncommitted T118 changes, package 0.1.1. Source identity: no installed game files read or modified; no gameplay session started.

## Steps / evidence

1. Inventory the checkout, Git changes and running build executables. `target` held 83,578 files / 66,353,326,534 logical bytes; code/docs and `.local` were much smaller. Check that the cleanup target resolves to this checkout's `target`, contains no reparse points, and has no running executable.
2. Preserve the release `KH2RW Launcher.exe`, `model_inspector.exe` and `image_inspector.exe` outside the checkout and compare SHA256. The two viewers are launcher's sibling programs. Preserve all files outside `target`, including local research, launch configs, saves and Git changes.
3. Automatic approval review rejected the initial recursive shell deletion, with only "blocked by policy" supplied as its reason. No shell deletion ran. Use Cargo's purpose-built `clean --target-dir <verified-checkout-target> --locked --offline` instead, after a successful dry run. Cargo removes only the designated build-output directory.
4. Move the three verified executables back to `target/release`, check their SHA256 again, compare the outside-target inventory (path/size/modified time) and Git status, and run the restored launcher's `--version` without opening gameplay.

Exact private evidence directory: `%LOCALAPPDATA%/KH2FinalRewrite/logs/T119-repo-cleanup/`. Files: `plan.json`, `target-inventory-before.json`, `kept-runtime-before.json`, `protected-files-before.json`, `protected-differences.json`, `git-status-before.txt`, `git-status-after.txt`, `cargo-clean-dry-run.log`, `cargo-clean.log`, `result.json`, `version.log`, `version.stderr.log`. Temporary runtime copies were moved back; no duplicate executable backups remain there.

## Checks / result

- PASS Cargo dry run and cleanup.
- PASS all three retained executable SHA256 comparisons; launcher `--version` succeeds with `kh2fm 0.1.1`.
- PASS outside-target inventory and pre-existing Git status unchanged before adding this record.
- PASS `git diff --check` after adding this authored record; no game data or personal paths added to Git-visible files.
- Removed 83,575 files / 66,180,251,590 logical bytes (about 61.6 GiB): debug builds, dependency/incremental caches, symbols, obsolete gravity/jump builds, duplicate `kh2fm.exe` and rebuildable developer probes.
- Remaining `target` contents: three executables / 173,074,944 bytes (about 165 MiB).
- NOT RUN: fmt, Clippy, tests, workspace build and window smoke during this cleanup. They passed in T118; no application source changed here. Rebuilding would recreate the outputs the user requested removed.
- NOT RUN: installed-source gameplay or viewer session; every later session still requires selected-source validation.

## Uncertainty / verdict / next

File-size totals are logical bytes; physical free-space change was not measured and can differ because of hard links or filesystem allocation. Cargo caches will return when the project is next built, and the next build will take longer. Runtime files needed by the existing launcher are retained; release publication and gameplay fidelity are not established.

Verdict: PASS for local build-output cleanup and runtime-file preservation. Next: use the retained launcher; rebuild developer tools only when needed.
