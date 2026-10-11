# T104 — Settings pages in the camp's Config (graphics, sound, other)

Owner Claude, 2026-10-11. Gate: none (a rebuild convenience, not an exe rule). Starts from the camp Config screen (T079 / camp-04 §3) and T100.

## Request

The owner asked for an in-game settings menu as a tab of Config, drawn with the game's UI assets: graphics, audio and other options such as Japanese text, all applying in real time.

## What the exe has and what this adds

The exe's Config (state 0x24, FUN_140364d70) has 8 rows stored in save+0x41a4 / +0x2498. Those stay page 0, unchanged. The PC options of the Steam build live in its separate launcher/settings menu (`Image/SettingMenu.hed`), not in the camp, so the pages below are the rebuild's own and claim no exe behaviour.

## Implementation

**`windows_shell::settings`:** the settings model (display mode, V-Sync, render scale, 4x anti-aliasing, brightness, master / music / effects volume, background sound, text language, HUD, mouse camera speed), its rows per page and Left / Right stepping (choices wrap, numbers stop at their ends). It is saved as JSON; an unreadable file gives the defaults, and out-of-range values are clamped.

**`camp` Config:**
- Page 0 is the exe's page. Pages 1–3 are Graphics, Sound and Other.
- L2 / R2 turn the pages. Tab 1 names the page, and the Status browser's "Switch" banner (sys.bar 0xb61a) shows beside it.
- The rows reuse the exe's Config panel, label, choice and cursor-frame groups.
- A row with up to 3 choices shows them all. A longer one (numbers, render scale, language) shows the current value, as Game Mode does.
- The help line is the row's own text.
- `encode_text` puts ASCII into the system font's encoding: lower case 0x9a.. after OpenKH's international table.
- `Camp::open_on_config` reopens the camp straight on Config, on the remembered page and row, after a language swap.

**`audio_out::Mixer::set_gains`:** a music gain for the given keys and an effects gain for every other voice and the stream. Volumes are squared, so 5 is −12 dB.

**`test_room::settings_room`:**
- Window mode and present mode follow the settings.
- The scene camera (order −2) gets `Msaa::Sample4`.
- The scene target is sized to the window × the render scale.
- The scene's composition colour is the brightness factor (0.6–1.5).
- The field HUD is hidden when HUD is off.
- The mouse turn per pixel is 0.02 × speed (0.1 at 5, the room's old value).
- The mixer gains follow, and are zero while the window is unfocused if Background Sound is off.

**Text language:**
- The languages offered are those whose `msg/<l>/sys.bar`, `fontimage.bar`, `fontinfo.bar` and `menu/<l>/camp.2ld` the selected source holds, found by name hash across the six KH2 indexes.
- A change reloads the camp source, the journal source, the system font and the field command menu (`field2d/<l>/<world><n>command.2dd`) in that language. Each configured file is relocated by matching its hash against the known language paths. A file the language lacks stays as configured.
- The open camp is then rebuilt on Config with its state carried over.
- Notice and photo files use `menu/<l>/`, falling back to `us`.

Settings are kept in `kh2fm-settings.json` beside the room's configuration (the private `.local/test-room/`). Unattended runs use the defaults and save nothing. The unattended walk now presses R2 in Config (tick 2405), so the Graphics page is in its captures.

## Install observation

The selected install (Steam build 15194255, English) holds KH2 text for `us`, `fr`, `gr`, `it` and `sp`, plus `menu/uk/` layouts. It has no `msg/jp/*` or `menu/jp/*` file in any KH2 index, so Japanese is not offered. The probe is in this task's evidence notes.

## Evidence (private, session scratch directory, not committed)

- A temporary walk (arena-07, release build, `--frames 2500`, not committed) captured:
  - 35: the Graphics page with the Switch banner and its key caps;
  - 36: Brightness raised to 8;
  - 37–38: the Sound page with Master Volume lowered, then Other;
  - 39–40: after Right on Text Language, the camp back on Config in French ("Options", "Changer", "MUNNIES", "TEMPS"), on the same page.
- The log has `{"event":"language","language":"fr","tick":2453}`.
- The committed walk (arena-07 via `kh2fm room --autoplay`) gave exit 0, 134 captures, none uniform.
- Twilight Town (`test-room-tt.json`, `--frames 200`) gave exit 1, 2 captures, and 123 entity-despawn warnings. That matches the pre-change `kh2fm.exe` on the same arguments (the frame limit ends the walk unfinished).

## Checks

PASS fmt; PASS clippy; PASS tests (new: settings rows, steps, derived values and clamped loading; mixer gains); PASS build; PASS smoke (`--frames 180`); PASS the arena walk.

NOT RUN:
- Exclusive fullscreen, V-Sync off, and anti-aliasing on a live window. The walk keeps the defaults; the code path is Bevy's window and Msaa components.
- The volume levels by ear.
- German, Italian and Spanish swaps (French only).

## Uncertainty

- Labels and help are English in every language; only the game's own text follows the language.
- Talk and field dialogue (area message bars) and the HUD's item / magic names keep the start language.
- Exclusive fullscreen uses the monitor's current video mode.
- 4x MSAA needs the adapter's support (wgpu's guaranteed sample count is 4).

## Verdict / next

PROGRESS: Config has Graphics, Sound and Other pages that apply as they change, and the menus swap text language in place. Next: the owner's review in the room. Japanese needs a source that has `msg/jp`, which this install does not.
