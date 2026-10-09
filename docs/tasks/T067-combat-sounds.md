# T067 — Combat and menu sounds

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T066 (`a92b755`). User request: keep rewriting the entire combat system.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence:
- Spec: `research/playable-room-20261008/sound-01/sound-spec.md`.
- Ghidra outputs: `research/ghidra-kh2-a3/snd-01`..`snd-08`; the gauge's alarm callers are in `ghidra-kh2-a1/hud-04`, `hud-05` and `ghidra-kh2-a2/hud-pred`.
- Trigger dumps: `sound-01/trig-dump2.txt`.

## Findings

1. **Request ids.** A request id selects bank `id / 1000` and sound `id % 1000` (FUN_1401de150). System sounds go straight to engine slot 2, which holds bank 0 (`se/se000.win32.scd`).
2. **Bank locations.**
   - Object banks are the `se/obj/seNNN` replacement assets of the object archive: Keyblade bank 500 in `obj/W_EX010.a.us`, Shadow bank 201 in `obj/M_EX020.a.us`.
   - The bank number is the SCD table header's u16 at +6.
3. **SCD layout.** Each sound lists {track, audio} layers, with a volume at +8 and the loop flag in attribute bit 0. Every stream used here is MS-ADPCM (codec 12), with 32 samples per block.
4. **Sound triggers.**
   - Frame trigger 8 plays `p0·1000 + p1` (FUN_14040dd70). With more than three parameters and p3 set, it plays only when p0 is the equipped weapon's bank.
   - The Keyblade's swing sounds live in `W_EX010.mset`: ground 500035/36/39 and air 500035/44/45 for this combo.
   - The Shadow's claws play 201020 at frame 30 of motion 201 and frame 50 of motion 202.
5. **Hit sounds.** An ATKP row's HitSound (+0x1C) plays on a hit when it is non-negative (FUN_1403d12c0). Sora's rows give 500000/1/4. The Shadow's claw rows give −1, so no sound.
6. **Low-HP alarm.** FUN_14017f050, which the gauge runs every frame, starts system sound 9 (looped) when HP ≤ MaxHP/4 (FUN_1403c1d30). It stops the alarm when HP recovers, or when the actor's flag 0x9b8 bit 2 is set.
7. **Menu sounds.** Cursor moves play system sound 1.

## Implementation

- **`package_reader::scd`:** the SCD reader (versions 3 and 4) and an MS-ADPCM decoder, tested on synthetic banks and blocks. ATKP rows now carry `hit_sound`.
- **`audio_out` (new crate):** a software mixer (linear resampling, loops, voice limit) and Windows waveOut output.
  - The waveOut FFI is the workspace's only unsafe code. It is confined to `audio_out::winmm`. The crate denies unsafe code everywhere else.
  - No new dependencies were added.
- **`windows_shell::sound`:** loads banks from original entries or named replacements, resolves ids, and caches decoded streams.
- **`test_room`:** a new `sounds` config list.
  - Sounds play for Keyblade swings (weapon frame trigger 8), hits (ATKP HitSound), the Shadow's motion triggers, menu cursor moves, and the looped low-HP alarm.
  - Every request is logged as a `sound` event.

## Evidence (private, outside Git)

`research/playable-room-20261008/autoplay-35.jsonl` with config `test-room-arena-02.json`:
- Banks 0, 201 and 500 loaded, and the device opened at 48 kHz.
- Ground combo: swing sounds 500035 → 500036 → 500039, hit sounds 500000/500001/500004 (volume 0.8 on the finisher), damage 3/2/6.
- Air combo: swing sounds 500035/500044/500045.
- Menu: cursor sound 1 at tick 981.
- Shadow claws: 201020 at ticks 119, 1190 and 1667, and Sora's hits stay silent.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS 180-frame smoke; PASS autoplay-35 (sound events as above).

NOT RUN:
- Listening comparison against the installed game.
- The low-HP alarm, since autoplay HP never dropped to 5 or below.
- An interactive playtest.

## Uncertainty

Inferred or test-room choices:
- The track opcodes are not interpreted. Random pitch/volume (opcodes 0x09/0x08) is not applied, so every sound plays at pitch 1.
- 3D attenuation, buses, priorities and voice limits are not modelled; sounds play centred.
- MS-ADPCM uses `>> 8` rounding; the Sd engine's rounding is unverified.
- KO stands in for flag 0x9b8 bit 2 when stopping the alarm.
- The alarm stops without its 0x3c fade.
- The weapon bank short is taken as 500 (identity for the Kingdom Key).
- Per-controller duplicate-id suppression (FUN_1403c8280) is not modelled.

Not implemented:
- Footsteps (trigger 2; the floor-material mapping is unresolved).
- Sora's voice (trigger 14).
- Effect-driven sounds, including the Shadow's appear/sink sounds 201016/17/19.
- 201021: it sits on the Shadow's JUMP motion at frame 30, which the jump claw replaces immediately.
- Decide/back/invalid menu sounds, since the Attack command doesn't use them.

## Verdict / next

PROGRESS: combat now plays the original's swing, hit, claw and menu sounds from the installed banks. Next: hit effects (waiting on the effect research), Guard, then the dialogue system with Axel.
