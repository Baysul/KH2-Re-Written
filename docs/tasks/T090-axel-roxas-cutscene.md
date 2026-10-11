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

## Added in the second pass

- Effects from `tt_event_613.pax` (effect elements 0, 1, 4, 5, 6) through new `effect_vm` programs (VtMime, MatrixZXY, DrawMatrix, DrawMdl3, DrawMdlTs3, VertexAp, MegaBirthModel, MngFlag) and an OMD mesh reader.
- WideMask bars are NOT drawn (owner request: no cinematic bars in cutscenes).
- Subtitles at 0.8x about the bottom-centre anchor; Keyblade actors hidden while the part list lacks group 0.

## Not done / uncertain (verdict: PARTIAL, not a 1:1 pass)

- Axel's hand fire (elements 7-9: ParMatrix, SMatrix, KeBornRnd6, VertexApLc, EiDrawShipoly), MegaBirthModel matrix modes 2/4/6/8, effect layer ordering, blend 0x88, PS2 gamma-space additive blending, HD textures; AttachEffect/StopEffect and end types 2/3/5 are ignored.
- SeqBlur feedback pass (equation known: out = prev*a/128 + cur*(1-a/128), previous frame zoomed ~1.004) and SeqTextureAnim (talk/blink patches) not built; SeqPart is only applied to the Keyblade actors, not Axel/the Roxas copy mesh groups.
- [U] music after the event ends and what clears the black fade; pause menu art (plain list, not pause.2ld); cursor sounds.
- No comparison against a real capture of the original.

## Third pass (owner feedback: Axel backwards, Keyblades not on Roxas, no bars)

- Keyblade actors now ride Roxas's bones 63 / 172 (AttachEffect type 1, `Timeline::attachments`).
- The composed scene is mirrored in x (the exe world is left-handed, the render right-handed); this turns Axel (and the whole scene) the way the original shows it. [I] confirmed by framing, not by an original capture.
- Letterbox bars removed.
- Partial worker output kept: SeqBlur feedback pass, fade layers, TEXA mouth/eye animations, mesh-group visibility (compiles, unit tests pass; not independently checked frame by frame).

## Fourth pass (owner: Axel opposite direction, Roxas hands, Axel summon effect)

- Actor yaw: the script heading is applied with the opposite sense (ry -> -ry, 180 unchanged) and the actor turns about its body centre, so Axel faces Roxas/camera in the talking shots (frames ~168-341 and 713-745 checked by capture).
- Keyblades: the attach matrix includes the exe's y/z flip (diag(1,-1,-1,1), FUN_1401a7af0 / FUN_1403ccb90); both blades now sit in Roxas's fists. The grip is not a separate pose, the fingers come from Roxas's own motion.
- Axel's summon effects: elements 7/8/9 start from the h_ex520 motion trigger records (ETT61305A: ticks 210, 210, 246 at 60 Hz, so event frames ~851/869), via new effect programs (SRandFV, SMatrix, ParMatrix, KeParMatR, KeDMatFr, VertexApLc, KeBornRnd6, EiDrawShipoly ribbon).
- Open: ribbon pair mode, layer draw direction (inferred), fade-rate of EndType 3, frame ~700 summon blade scale, compared to no ground-truth capture.
