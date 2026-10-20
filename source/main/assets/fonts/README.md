# Launcher fonts

The launcher draws its own text. On Windows it uses the system's Trebuchet MS
and Segoe UI. Elsewhere it uses these two bundled faces, built into the
launcher with `include_bytes!` (`source/main/src/launcher/paint.rs`):

| File | Used for | SHA-256 |
|---|---|---|
| `LiberationSans-BoldItalic.ttf` | Headings | `698da70fc191cc5f33ad4d6d3fe830fe4624b898ea2e3169955928b7c491f1ee` |
| `LiberationSans-Bold.ttf` | Menu rows and body text | `788abee4c806d660e8aee46689dd8540cd4bb98da03dcc9d171ce3efd99a9173` |

They are unmodified files from the upstream
[Liberation Fonts 2.1.5 release](https://github.com/liberationfonts/liberation-fonts/releases/tag/2.1.5)
(tag commit `4b0192046158094654e865245832c66d2104219e`), taken from
`liberation-fonts-ttf-2.1.5.tar.gz`
(SHA-256 `7191c669bf38899f73a2094ed00f7b800553364f90e2637010a69c0e268f25d0`).

Copyright (c) 2010 Google Corporation, with Reserved Font Arimo, Tinos and
Cousine; copyright (c) 2012 Red Hat, Inc., with Reserved Font Name Liberation.
Licensed under the SIL Open Font License 1.1; the full text is in
[docs/release/liberation-fonts-LICENSE.txt](../../../../docs/release/liberation-fonts-LICENSE.txt)
(trailing whitespace normalized, text otherwise unchanged). The fonts are not
renamed or modified; a modified version could not keep the reserved names.

These are launcher fonts only. They are not the game's fonts and say nothing
about in-game text, which comes from the selected installation.
