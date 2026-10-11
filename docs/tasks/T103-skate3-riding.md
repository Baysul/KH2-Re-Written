# T103 — Skate 3 riding on J

Owner Claude, 2026-10-11. Gate: none (owner-requested exception, as T101). Prerequisites: T101 (disc admission), the owner's Skate 3 ISO.

## Source and build identity

- Disc: the owner's Skate 3 (USA, Europe) Xbox 360 ISO, 7,838,695,424 bytes, admitted by `skate_source` (layout check only; no retail digest established).
- Skate engine: SK8-ENGINE/skate-3-rust-engine at `2e16697`. Owner's choice (2026-10-11): inside this repo as an optional feature. Two GPL-3.0-only crates in `crates/skate3/` (README and LICENSE there), built only with the `windows_shell` feature `skate3`; `skate-core`, `skate-data` and `skate-dynamics` are pinned git dependencies:
  - `skate-disc`: the core asset setup (`asset_exports.core`) in Rust: EB BIG v3, chunkref, RefPack, AttribSys VLT to `skater-collections.json`, ABIN physics skeletons.
  - `skate-sim`: skate-game's simulation (physics, graphs, input, gameplay camera numbers) without rendering, UI, replay, modding, networking or SDL3, on Bevy 0.19.1; adds host collision worlds and a `Sim` entry point.

## Implementation (this repo)

- `windows_shell` depends on both crates through the optional `skate3` feature; without it J says skateboarding is not installed.
- J: admit the disc; prepare the riding data once into `<data dir>/skate3` on a worker thread (keyed by ISO path, size and time); build the sim on the room's collision triangles (scene units / 100, two-sided) at Sora's position and facing; step it at 60 Hz with keyboard or pad as an XInput pad; J again gets off. An area change ends the ride.
- Sora: control is locked while riding; his position and yaw follow the rider root. `skate_rig` gives each mapped Sora bone (legs, feet, spine, neck, arms, hands) the rider bone's rotation change since its rest pose, after a fixed correction from Sora's T-pose bind to the rider's A-pose rest; joints keep Sora's bone lengths and the pelvis sits on the rider's hips. He is skinned on the CPU with those matrices (`Skinning::positions_with`). Bone choices read from P_EX100's bind pose.
- Score HUD: skate-game's trick-display player (`hud_runtime`, `apt_*`, Bevy-free) runs in skate-sim beside the scoring runtime (`host_hud::ScoreHud`), stepped after each sim tick. skate-disc's `hud` module prepares its data from the disc (`fedata.big` / `fetexture.big` / `miscboot.big`, the `hud2/trickdisplay2` movie, its RW4 textures, bitmap fonts and the language table) into `<cache>/private/hud/runtime`, ported from `tools/prepare_hud.py` and `tools/vendor/skate3_ui` (MIT). `skate_hud` draws it as textured triangles (texel x multiply + add) on its own overlay camera in the movie's 1280x720 space; the KH2 command menu and gauge are hidden while it shows.
- Sounds (owner chose vgmstream, 2026-10-11): on the first ride kh2fm downloads the pinned vgmstream r2117 release with curl.exe, checks its SHA-256 (6c4a8a38...dc6c, the digest the Skate 3 engine's setup pins) and unpacks it with tar.exe into `<cache>/tools`. skate-disc's `audio` module (ported from `tools/asset_pipeline/audio_export.py`) then converts the skater's sounds on a background thread (grains, wheel streams, 28 banks, Csis projects, ABKC banks, Splice trees, MixMap, tuning) into `<cache>/private/audio`; riding does not wait for it. skate-sim's `audio` (skate-game's `game_audio` player path: skate_events, player_audio, grain_bed, native clocking, library) drives skate-audio each tick; its 48 kHz stereo stream is mixed over the room's voices (`audio_out::Mixer::set_stream`). World ambience, NPC skaters, world emitters, speech, water and front-end sounds are not carried.
- Camera: while riding, Skate 3's gameplay camera (skate-sim's `CameraRuntime`, its transform and field of view, fed the window's aspect) replaces the KH2 follow camera.
- Board: Sora's equipped Keyblade (owner's choice, 2026-10-11). `keyblade_board` resolves item -> model as the game data links them: the item record's +4 is a position in a 03system WENT set (counting the set's size word as 0), the set is the one P_EX100's object entry names by its neo-moveset index, and the entry is an object id whose model is `obj/<name>.mdlx` (layouts per OpenKH's 03system/00objentry notes; on the install the Kingdom Key resolves to W_EX010, the room's own weapon, and Oathkeeper to W_EX010_10). The model is read from whichever index holds it, laid flat on the deck (longest extent along the board), centred under the feet and scaled to a 1.4 m longboard (the first version's 0.8 m deck scale left it too small for Sora's feet); re-equipping mid-ride swaps it. The room's own Keyblade reuses the hand meshes, which are hidden while riding; a plain cuboid is the fallback. Scale: 100 scene units per metre (Sora's model is 171 units tall).

## Checks

- skate-disc vs the Python setup on the owner's disc: PASS. 2029 extracted files byte-identical, 6849 collections equal, PHYS_TPOSE skeleton equal (private cross-check script, outside both repos).
- Headless sim from the prepared data: PASS. Pushing accelerates the skater; a host wall stops and returns it.
- Test room, scripted J and pushes (temporary code, not committed), arena-07: PASS for riding, wall approach and the notice. Sora's retargeted pose: seen only from the follow camera; close inspection NOT RUN.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked`: PASS, each with and without `--features windows_shell/skate3`. skate-sim's carried code is exempt from clippy style lints (its manifest); unsafe code stays forbidden.
- Test room with the Skate camera and full-rotation retarget, scripted J (temporary code): PASS by capture; close-up pose review NOT RUN.
- Release build of `kh2fm` with `skate3`: PASS.
- Sound reaching the device (2026-10-11): an owner's ride closed mid-conversion left a partial `private/audio` with no manifest, so no sound loaded. The conversion now builds aside (`audio-out.tmp`) and moves into place only when complete; a notice says when sounds are still being prepared. Test room, scripted pushes, logging what the output device pulls from the stream each second: -32 to -42 dBFS RMS, peaks up to -9.6 dBFS while riding: PASS.
- Sounds on the owner's disc: 3,016 files (169.5 MB) prepared in about 150 s; manifest read back with library.rs's types; 2,961 WAVs valid. Headless 10 s push ride through skate-sim's audio: non-silent every second (RMS -31 to -38 dBFS). Test room: sounds load with the ride. Listening check and equivalence with skate-game's output: NOT RUN.
- Score HUD on the owner's disc: prepared (18 textures, all width x height x 4; 87 characters; 13,607 language entries); test-room capture with scripted pushes and right-stick ollie flicks shows "Ollie", the sequence score and the stance ring: PASS. Byte equality with the Python HUD output: NOT RUN (the Python reference was not executed; the port was reviewed function by function).

## Uncertainty

- A build with `skate3` is GPL-3.0; the default build stays Apache-2.0.
- Fingers and face are not retargeted; the bone map and the rider rest pose (its pose before the first step) are inferred, not named data.

## Verdict

PROGRESS: riding works in the test room from the owner's disc.

## Next

Owner playtest of the ride, the score HUD and the sounds.
