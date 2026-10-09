# T075 — One gamma-space 2D composition over the rendered scene

Owner Claude, 2026-10-09. Gate P22. Starts from T074.

## Why

The exe draws its 2D layers (HUD, message windows, menus) straight onto its gamma-encoded framebuffer. In T073 the camp was laid over the scene by a second camera, and that composite ran in Bevy's linear window space. Semi-transparent pixels such as the 50 % navy and black backdrops therefore came out lighter than the exe's.

## Implementation

- `test_room`:
  - The 3D camera renders into an sRGB image the size of the window, and a resize system follows the window.
  - The single 2D layout camera (`ui2d`, cleared black) composes, in order: the scene, the talk dissolve snapshot at its fading alpha, the HUD canvas (hidden while the camp is open [inferred]), and then the camp.
  - Everything blends in gamma space.
  - The HUD's own 2D camera and its sprites are gone.
- `ui2d`:
  - `add_image` registers existing images such as render targets.
  - sRGB sources are re-encoded to their stored gamma values in the shader (`SRGB_SOURCE`).
  - `full_screen` draws a whole-window quad.

## Evidence (private, outside Git)

- `autoplay-52` (arena-05):
  - capture 09: the combat HUD, command menu and hit effects over the scene;
  - capture 22: the camp with its darker gamma-blended backdrop.
- `autoplay-53` (talk-02), capture 04: the talk close-up with the window and HUD.

Both runs exit 0 with the same event logs as before.

## Checks

PASS fmt; PASS clippy; PASS tests; PASS build; PASS smoke; PASS autoplay-52/53.

## Uncertainty

- The 3D pass itself still blends in linear space: effect quads and transparent model parts. Bevy's PBR pipeline has no gamma-compositing mode.
- The HUD canvas is still rasterised on the CPU: gauge masks need its depth trick.

## Verdict / next

PROGRESS: all 2D now composes like the exe's framebuffer. Next: music (a Vorbis decoder), the camp's party models, sub-screens.
