# T181 — Test-room scene stops updating after the window is resized or maximized

Owner: coding agent. Gate: native-platform development defect fix only; no source qualification, gameplay, fidelity or compatibility gate advances. Prerequisites: T177–T180. Trigger: in the user's own session (T180 step 10), with the test room started from the launcher on the Epic copy **E01**, maximizing the window froze Sora, Donald and Goofy on screen while combat went on: attacks still landed, and the menus and HUD still changed.

## Scope and identity

Reproduce the freeze, find its cause and fix it without changing any rendering rule. No test is added or changed; no game data enters Git.

Baseline `554d3bf18d3bf7be855ab4f311c3c0b7aeb1a34f` (branch `epic-linux`) plus the uncommitted T177–T180 changes; Rust/Cargo 1.99.0, Bevy 0.19.1. Source: **E01**, still **quarantined/unqualified**; its identity and file details are private. Host: x86_64 CachyOS, Wayland, AMD Radeon RX 6700 XT, RADV, Mesa 26.2.4, Vulkan.

## Evidence locations and method

Private evidence bundle **T181-B01**, outside the checkout: copies of the generated test-room configs and manifest, the run script, a frame-difference script, one log per run, all screenshots and the session's app logs, check logs and job receipts. Every GUI run used a private `kwin-mcp` session (1920x1080, isolated home), never the user's desktop. The source was only read.

The feedback loop starts the combined test room from E01 at its default 1280x720 window. Readiness is the program's own "First frame" log line. The loop takes two screenshots 0.4 s apart and measures the mean absolute pixel change in three boxes of the window's client area: the party, the scene centre and the command menu. It then changes the window (KWin maximize button, or a direct resize to a 1600x914 client), takes two more screenshots 0.4 s apart, and quits with Esc. A run is **RED** when the party moved before the change (above 0.3) and not after it (below 0.05), and **GREEN** when the party still moves after it (above 0.3). A control run makes no change.

## Steps and observations

1. **Reproduced.** Before maximizing, the party box changed by 12.41 (mid-attack). After maximizing to a 1920x1044 client, the party and scene-centre boxes changed by 0.00 while the command menu still changed (1.90, 0.77). Frames five seconds later still showed the 3D scene frozen; the changed pixels were the 2D layers plus one small area near Sora, whose source was not identified. Restoring the window size brought movement back in that run (party 1.55); why it recovers was not established.
2. **Feedback loop.** Resize runs went **RED** twice and control runs **GREEN** twice on the unfixed build. A first batch is discarded: a script edit had not applied, so its "control" runs also resized the window.
3. **Cause.** The test room draws the 3D scene into an image, and the 2D pass composes that image with the HUD (`test_room.rs`, `resize_scene` and `compose_2d`). When the window size changes, `resize_scene` resizes the image, which gives Bevy a new GPU texture. The 2D pass draws the image through `LayoutMaterial`s that its frame caches per sheet and blend (`visual::overlay::Ui2dFrame`). Bevy 0.19.1 prepares a 2D material's bind group from the image at that moment (`PreparedMaterial2d::prepare_asset`). It prepares it again only on the material's own asset events (`bevy_render` `render_asset.rs`, `extract_render_asset`). The cached materials therefore keep sampling the old texture, which holds the last picture drawn before the resize. The HUD sheets are separate textures, so they keep updating. The Drive afterimage's feedback materials (`cutscene/blur.rs`) sample the same scene image and would go stale the same way.
4. **Probe.** A temporary change that replaced the scene sheet's cached materials after each resize (`Ui2dFrame::rebind`) turned all four runs **GREEN** (two resizes, a maximize and a control). It was removed.
5. **Fix** (`test_room.rs`, `resize_scene`). After resizing the scene image, every `LayoutMaterial` whose texture is the scene image is marked changed (`Assets::get_mut(..).into_inner()`; a bare `get_mut` does not mark it), so Bevy prepares it again against the new texture. This covers the 2D pass and the afterimage, and runs only on frames where the size changes.
6. **Verified.** With the final build, four runs went **GREEN**:

   | Run | Client after | Party before → after | Scene centre after | Command menu after |
   | --- | --- | --- | --- | --- |
   | Resize | 1600x914 | 9.27 → 28.29 | 27.24 | 1.15 |
   | Maximize | 1920x1044 | 7.40 → 34.25 | 28.67 | 0.95 |
   | Control | 1280x720 | 57.85 → 39.07 | 30.06 | 1.68 |
   | Resize | 1600x914 | 57.65 → 40.40 | 26.88 | 1.76 |

   The large "before" values in the last two runs are the room's opening fade, still running at the first screenshot. A maximized frame was inspected: the scene fills the window, is sharp, and the party pose differs from the baseline. Every run exited with code 0. The run logs hold no validation, error or panic lines.
7. **Other render targets.** Only the test room resizes an image at run time. The event player's scene image and the afterimage's two outputs keep their startup size. They cannot freeze this way, but they are upscaled when the window grows; that is unchanged. The menu-model and dissolve-snapshot targets are fixed at 1280x720 by design.

Side observations:

- The isolated session logged "Gamepad connected": a host gamepad reaches programs inside the private session. The measured runs were unattended, but host controller input could reach a test run.
- The runner blocks a fifth consecutive identical tool call, and that block stopped the agent's run while it polled for the window. The final loop makes one geometry call after the baseline screenshots and stops if no window is found.

## Checks

| Check | Result |
| --- | --- |
| Freeze reproduced on the unfixed build (maximize; resize ×2; control ×2) | RED as expected for the window changes; controls GREEN |
| Final fix: resize ×2, maximize, control | PASS: all GREEN; exit 0; no validation/error/panic lines |
| Final loop order (geometry read after the baseline) against the unfixed build | NOT RUN: it differs from the RED runs only in when the window position is read |
| Afterimage (Drive change, death blur) after a resize | NOT RUN: reaching a Drive needs repeated menu input; covered by code review only |
| Resolution setting changed during a run (same `resize_scene` path) | NOT RUN |
| Window made smaller than the default | NOT RUN |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS: 725 passed, 0 failed, 1 ignored, 28 test programs |
| `cargo build --workspace --locked` | PASS |
| Windows-target clippy of `main`, all targets, warnings denied | PASS (compile-only cross-check; `CARGO_FEATURE_PURE=1`, as in T178) |
| Native Windows | NOT RUN: no Windows host. The defect and the fix are in shared code, so Windows builds very likely froze the same way (inferred, not observed) |

## Uncertainty, verdict and next work

**PASS** for this fix: the test-room scene keeps updating after a resize or maximize on Linux/Wayland, and the workspace checks pass. No gate advances, and the NOT RUN rows above stay open.

The required checks ran with a private build folder, as in T180.

Limits: the loop measures pixel change in fixed boxes, so it detects a frozen scene, not a wrong picture; the maximized frame was checked by eye once. Why restoring the window size un-froze the unfixed build was not established; the fix does not depend on it. A useful regression test, needing approval first: a headless Bevy app that registers a material sampling the scene image, changes the window size, runs `resize_scene` and expects a `Modified` event for that material.

Next: the user can check the afterimage after maximizing (start a Drive, or let Sora be defeated). Remaining port work is listed in T180.
