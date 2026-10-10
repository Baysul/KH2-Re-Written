# T090 — Axel and Roxas cutscene (tt19 event 613)

Owner Claude, 2026-10-10. Gate P23 (events). User request: remake the Axel/Roxas scene before their fight, where Roxas draws two Keyblades, 1:1 from the exe, with cameras, models, animations, subtitles, audio, music and a pause menu with Continue and Skip Scene.

## Source

Steam app 2552430, build 15194255, content identity `a9ca1e03…`, quarantined; manifest `.local/test-room/source-manifest-02.json`. Private evidence (outside Git): `%LOCALAPPDATA%\KH2FinalRewrite\research\playable-room-20261008\cutscene-01\` (core, camera, audio, effects specs and Ghidra output).

## What it is

`ard/us/tt19.ard`, BAR entry type 22 named `613` (`tt_event_613`): SetEndFrame 1093 at 30 event frames/s, 11 cameras, actors Roxas (P_EX110_BTLF), Axel (H_EX520), Oblivion/Oathkeeper (W_EX010_10/_20), a Roxas copy (B_EX110), five voices, music151, SE bank 2933, five subtitles (msg/us/tt.bar 0x3528–0x352c).

## Implementation

- `package_reader::event`: script decoder; `named`: files by path name (from the Twilight Town thread's commit); `model_inspection::read_models`, `motion::read_anb`.
- `event_sim`: timeline (cuts, actor tracks, subtitles) and camera key evaluation (1/512 s keys, Hermite, extrapolation).
- `kh2fm event` (`bin/kh2fm/cutscene.rs`): playback, pause menu, launcher entry "Cutscene: Roxas and Axel".
- Exe-derived rules applied: SeqActorLeave despawns; motion holds or loops by FrameReturn; camera cuts last-wins with y/z negation; voices at their frame, SE as bank*1000+n, music151 at frame 336 level 0x3000; subtitle colour 0x80f0f0f0; Start opens Continue / Skip Scene, which freezes clock and sounds; Skip Scene ends the event.

## Checks

- PASS fmt, clippy, test (workspace, locked); release build.
- PASS windowed captures of frames 60–1000 (cameras on Axel and Roxas, subtitle at the exe position).

## Not done / uncertain (verdict: PARTIAL, not a 1:1 pass)

- NOT DONE effects (tt_event_613.pax: ~12 effect program types missing in `effect_vm`, OMD model renderer undecoded), SeqBlur, WideMask bars, SeqTextureAnim, SeqPart visibility lists.
- [U] fade curve/colour, music after the event ends, pause menu art (plain list, not pause.2ld), exact subtitle vertical transform, pause cursor sounds.
- Audio not compared against a real capture; no in-game reference run.

Next: effects programs, WideMask/blur, pause.2ld layout, then comparison against the original.
