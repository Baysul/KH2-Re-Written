# T110 — Smooth area transitions and faster loads

Owner Claude, 2026-10-11. Gate: presentation pacing (P16/P24 rooms and streaming; no gameplay rule changes). Starts from `main` at cfde681. User request: room transitions smooth instead of choppy, less lag, faster loading. Steam build 15194255 through the session-validated source (`source-manifest-02`), Twilight Town at the T097 story point.

## Measurement

Release `kh2fm --features skate3`, `kh2fm tt` with `test-room-tt-01.json` (Station Plaza, explores to tt06) and `--autoplay`, on the owner's PC with other work running. The run log now records `first_frame`, `area_load` (worker and main-thread ms), `hitch` (any frame over 50 ms, with the transition stage) and `frame_stats` (per 600 frames: mean, worst, frames over 20 ms). A `bevy/trace_chrome` build (local only, not committed) gave per-system times.

| | Before | After |
|---|---|---|
| Startup decode / first frame | 2.97 s / 3.73 s | 1.93 s / 2.65 s |
| Area change, main thread | 2.30 s in one frame | 10 ms (decode 1.6-2.2 s on a worker during the iris) |
| Iris and fade frames | 150-400 ms each, then a 2.8 s frame on arrival | none over 50 ms; one 91 ms frame as the new area first draws |
| Field play in tt06 (600-frame windows) | mean 38 ms, 555 of 600 frames over 20 ms | mean 10.0 ms, 0-9 frames over 20 ms |

Evidence (private, outside the checkout): `%LOCALAPPDATA%\KH2FinalRewrite\research\perf-01\` — `base-tt` (before log and captures), `after-tt2` (after), `trace-tt2\agg.txt` (span totals), `probe-tt*` (canvas timings).

## Causes and changes

- The next area was decoded on the main thread in one frame after the fade (`AreaSource::load`, 2.3 s), and the next frame's catch-up ran up to 15 ticks at once. `AreaSource::fork` (own indexes, copied story/random/music state) now decodes the area on a worker thread from the moment the exit fires; the screen holds black until it is done, then `adopt` takes back the fork's townsperson models, random state and music pair. The presentation is unchanged: iris close (71 frames), 2-frame fade, black while loading, fade in with the iris opening [exit-01 §4]. While loading and opening, a slow frame advances at most two ticks (`transition_time`), so the iris and fade no longer skip frames.
- The iris and fade were rasterised in software into the full-screen HUD canvas (60-200 ms per frame). They now draw on the GPU through the 2D layer (`Ui2dFrame`) over the HUD, same quads, order and colours.
- The software canvas (`gauge::Canvas`) draws large frames in row bands on several threads; each pixel still sees every draw in order (test `banded_frames_match_one_thread`), and the pixel loop no longer calls through `dyn Fn`.
- Map chunks with the same texture and blend share one material (UV-scrolled chunks keep their own), so the renderer batches them; the UV scroll only writes a material when its offset changes.
- Townspeople and map-object meshes are re-posed once per tick instead of every rendered frame; hidden townspeople are skipped.
- Startup decodes the area on a worker thread while Sora's kit (motions, 0.9 s) loads.

## Checks

PASS fmt; PASS clippy (workspace, warnings denied); PASS workspace tests (426); PASS workspace debug build; PASS release `kh2fm` build with `--features skate3` (last built before a comment-only encoding repair; the owner had that exe open afterwards). PASS unattended TT run: tt08 → tt06 exit, arrival and the capture set; the iris captures match the earlier ones by eye. The run still exits 1 because capture 1 (tick 110) lands on the black hold, as it did before this change.
NOT RUN: `cargo run -p windows_shell -- --frames 180` smoke (skipped while the owner was playing); a capture-for-capture comparison against the original's transition timing on PC.

## Uncertainty

- The black hold now lasts as long as the worker's decode (about 1-1.2 s after the iris on this PC). The original's hold length on PC was not measured.
- The field keeps ticking during the hold with the pad zeroed (FUN_1403b2340), as it does during the iris; before this change the hold froze the main thread instead.
- Per-frame CPU skinning still re-uploads Sora's, the party's and the enemy's meshes each frame (`allocate_and_free_meshes` about 2.4 ms a frame in the trace). The HUD canvas is still copied and uploaded whole whenever it changes.

## Verdict / next

PROGRESS: transitions in Twilight Town run without long frames and area changes no longer block the game. Next: GPU skinning or per-tick posing for the actors, a partial HUD upload, and a timing comparison of the original's black hold.
