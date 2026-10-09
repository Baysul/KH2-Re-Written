# T086 — Boot sequence: the scene-8 splash on every launch, plus the studio splash

Owner Claude, 2026-10-09. Gate P22 (front-end scope). Starts from T071. User request: make the title sequence 1:1 (the Square Enix logo and the copyright screen were missing) and add one splash of the user's own, "GLURBUS STUDIOS".

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Steam build 15194255. Private evidence:
- Boot spec: `research/playable-room-20261008/boot-01/boot-spec.md`, with 36 decompiled excerpts in `boot-01/excerpts/`.
- Title spec: `title-01/title-spec.md` (§2.3 title task, SEQD 1 in `title-us-layd.txt`).

## Findings (boot-01)

- **[O]** `axaAppMain` 0x14014f1e0 builds the app. Init FUN_14014ed40 runs once per process. It calls FUN_140150870, which sets first-boot bit0 of DAT_140716980 from DAT_140716614 (1 in the image, no writer). It then registers the title as the first program.
- **[O]** No other program, movie, logo, legal or "press start" screen comes before the title. **Not traced:** the framework run loop FUN_140105420.
- **[O]** FUN_1401508a0 clears the bit when the title picks scene 8. So the Disney / Square Enix / copyright splash plays once per process start, unconditionally.
- **[O]** Order on a first launch:
  1. `fade.out(0)`, then `fade.in(0x3c)` over an empty black screen.
  2. The controller check FUN_1402b64e0 and the storage check FUN_1402b53e0.
     - **[I]** Both show nothing on PC. The controller notice tests an app byte that is never 2. The storage check passes unless 100 saves exist.
  3. The main layout starts on scene 8 with next = 12/13; the Title BGM starts one yield later.
  4. Scene 8 runs 851 frames, then switches straight to 12/13. Scene 17 is not played on a first launch.
- **[O]** The skip mask 0x404 is tested only in scenes 12/13, so nothing skips scene 8. No SE plays during it.
- **[D]** SEQD 1 timing (linear, backdrop 0xf5f5f5):
  - 0–19: black → backdrop;
  - Disney: 20–79 in, 80–199 hold, 200–259 out;
  - 260–279: backdrop only;
  - Square Enix: 280–339 in, 340–459 hold, 460–519 out;
  - 520–539: backdrop only;
  - copyright: 540–599 in, 600–719 hold, 720–779 out;
  - 780–850: backdrop only.

## Implementation

- `windows_shell::title`: no logic change. `Title::new(first_boot = true)` already started scene 8 with the intro queued. A new test checks that decide/START do not skip scene 8 and that it hands straight to scene 12.
- `kh2fm title`: `first_boot` now defaults to true, so the splash plays on every launch as in the exe. A config can still set it false. Unattended runs (`--autoplay`) keep their scene-17 start and skip both splashes, because their press plan uses fixed ticks.
- `windows_shell::splash` (new, **not original**): the user's GLURBUS STUDIOS splash, placed after the Square Enix logo at the user's request.
  - The boot splash holds at frame 520, on its bare backdrop, after the Square Enix logo has faded out at 519. The layouts and the title task get no frames while the studio splash plays; scene 8 then resumes at 520 with the copyright block. The Title BGM, already started under the Disney logo, keeps playing.
  - It copies the Square Enix group's timing: 20 frames of bare backdrop, then the logo under a fading overlay at 20–79 / 80–199 / 200–259. Total 260 frames.
  - It plays only with the boot splash, so a config with `first_boot: false` shows neither.
  - The logo square is drawn 224 units tall (the Disney part's height) at the centre. The image's paper is set to the backdrop grey 0xf5, so no box shows.
- Asset: `crates/windows_shell/assets/glurbus-splash.png` (1280×720).
  - **Provenance:** the user's own art. They uploaded it in the project thread and asked for it to be used. It is not game data.
  - It is allowlisted by name in `.gitignore` and embedded with `include_bytes!`.

## Evidence (private, outside Git)

- `title-run-15` to `title-run-18` (+ `.jsonl`). Runs 15–17 had the studio splash first; run 18 has the final order.
  - These came from a local, uncommitted build that kept both splashes in the autoplay plan.
  - Run 18 timeline: title start on scene 8 at tick 123, BGM at 124, studio splash from tick 645 (scene frame 520) to 905.
  - Run 18 captures show the Square Enix logo (tick 450), the studio splash on the matched 0xf5 backdrop (750), then the copyright block (970).

## Checks

- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- PASS `cargo test --workspace --locked`. New tests: splash 4, title 1.
- PASS build.
- PASS 180-frame smoke.
- PASS for this task's files: `cargo fmt --all --check` fails only on `crates/package_reader/src/journal_table.rs`, an uncommitted file from another thread.

NOT RUN:
- a comparison against a capture from the installed game;
- the controller and storage dialogs, which are not built and are inferred to show nothing on PC.

## Uncertainty

- **[U]** The clear colour behind the opening fade (assumed black).
- **[U]** Exact load/yield frame counts between the fade and scene 8.
- **[U]** The three sound calls before the BGM.
- **[U]** FUN_140105420 was not decompiled.

## Verdict / next

PROGRESS. A launch now plays the opening fade, then Disney → Square Enix → the studio splash → copyright with the Title BGM, then the intro and the idle title, matching the exe's first-boot order. Next: compare the sequence against an installed-game capture.
