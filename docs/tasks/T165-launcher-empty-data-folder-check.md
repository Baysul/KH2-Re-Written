# T165 — Launcher: every test from an empty data folder

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub issue
#35). Gate: development only. Prerequisites: T109 (launcher quick start).
Build identity: `main` at `c7aa907`, package 0.1.2, Rust 1.99.0 Windows MSVC.
Source: the selected Steam KH2FM build; the install was only read.

## What it checks

`launcher::install::tests::every_test_starts_from_an_empty_data_folder`
(ignored by default; set `KH2_TEST_FOLDER` to the Steam game folder and run
`cargo test -p main --locked every_test_starts -- --ignored` after building the
main binary) uses a fresh temporary data folder: it hashes the game folder into
a new manifest, makes the launcher's configs (`test_args`) for each entry of
the test list and runs the built `KH2RW` on each with a short unattended run
(`--frames 240`; the event player only loads, `--load-only`).

## Checks

- PASS on the installed build: the scan, the made configs and the five
  programs (Title screen, Test room, Twilight Town, The World That Never Was,
  Cutscene: Roxas and Axel) all started from the empty folder and exited 0
  (288 s, mostly the scan). The event player loaded its event.
- NOT RUN: the launcher window itself (its buttons and the quick start guide);
  an interactive first run on a clean machine.

## Verdict

PARTIAL. The programs start from an empty data folder; the launcher's own
screens still need an interactive check. Next task: that interactive check.
