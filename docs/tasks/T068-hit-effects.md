# T068 — Hit effects: effect data readers and the particle VM

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T067 (`c15ba25`). User request: keep rewriting the entire combat system.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence:
- Spec: `research/playable-room-20261008/effect-01/effect-spec.md`.
- Ghidra outputs: `research/ghidra-kh2-a4/fx-01`..`fx-26`.
- Dumps: `effect-01/dumps/` (W_EX010 DPD1, M_EX020, texcommon).

## Findings

1. **Hit sparks.** The spark comes from the attacking object's PAX (FUN_1403d1b40).
   - For the combo, the attacker is the Keyblade object, so the container is `obj/W_EX010.a.us` PAX "w_ex010".
   - Lookup: the first element whose category contains `(1 << (32 + hitEffect − 1)) | 1`.
     - Effect 1 (rows 126/127) → element 1 → DPD1 PDATA0.
     - Effect 2 (finisher 129) → element 2 → DPD1 PDATA1.
   - Placement: the contact point on the target's hit shape, rotated to the hit direction. At most 8 hit effects live at once.
2. **Shadow death.** The `dead` act binds element id 2 of `obj/M_EX020.a.us` (DPD0 PDATA0).
3. **VM stepping.** FUN_1402c28e0 runs an update pass every game frame, with programs gated to even part frames. The draw pass (FUN_1404b09e0) advances part frames and instance time by 0x1000 every second frame, which gives a 30 Hz VM.
4. **Spawning.**
   - Groups (one per entry in the PDATA entry list) spawn at their start time.
   - Births (FUN_1404b02f0) are skipped until the next frame advance.
   - Parts die when their frame reaches the entry's total.
5. **Draw space.** FUN_1402c31e0 sets ec60 to the camera's view matrix, so ed60 = view·world. DrawMatrixFront keeps the particle axes in view space (billboards) and moves only the translation through ed60.
6. **Shape sprites.** Positions are X/16, Y/16 (FUN_1404ad7b0 / FUN_1404ada10). The colour bytes are R, G, B, A, then the GS ALPHA byte (0x48 add, 0x44 blend).
   - The flip-book draws the previous index, and reads the wait of the index before that (FUN_1404c9b30).
7. **Registration (FUN_1402c1610).** Shapes are registered by id with their anim header at +0x10. Model vertex arrays sit at model + 0x10 + [model + 0x18]. VSF records are {vsfNo, modelNo, n, indices}.
8. **Atlas.** The shapes sample `effect/texcommon.dpd` texture 10 (TBP 10752, PSMT8 256×448). Its PC replacement (the first DDS) is BC3 256×512 and matches the original rows (mean |ΔRGBA| 1.28), so UVs normalise by 256×512.

## Implementation

- **`package_reader::effect`:** readers for PAX, DPX, DPD (PDATA entries/programs, shapes, models, VSFs, PSMT8 textures with the CSM1 palette) and shape frames.
- **`effect_vm` (new crate):** the VM translated.
  - Instance, groups and parts, with work variables at their byte offsets and raw keyframe cursors.
  - Program types: Accele/Move (f32, i32, i16 families), Point, Angle, Scale, Color, RandFV, RandIV, MatrixXYZ, MatrixScl, DrawMatrixFront, KeZCrct, KeZCrctShp, DrawShapeX, KeBornRnd5.
  - The CRT `rand()` LCG and libvu0 matrix conventions.
- **`windows_shell::effects`:** loads object PAX archives and the atlas, using the replacement texture when it decodes.
- **`test_room`:** a new `effects` config.
  - A Keyblade hit spawns the ATKP hit effect at the contact point.
  - The Shadow's death spawns its bound death effect.
  - Every frame, quads are mapped back from view space and drawn in two meshes (additive and alpha), with gamma-space colours converted to linear.

## Evidence (private, outside Git)

`research/playable-room-20261008/autoplay-36` with config `test-room-arena-03.json`:
- Hits at ticks 419, 448 and 487 start effects 1, 1 and 2, and live parts and quads are logged.
- Captures `test-room-08.png` (tick 450) and `-09.png` (tick 495) show the star sparks at the contact points.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS 180-frame smoke; PASS autoplay-36. NOT RUN: comparison against an installed-game capture of the same hit.

## Uncertainty

- **Inferred:**
  - ede0 (the effect draw space) is treated as identity in view space.
  - The instance colour is (1,1,1,1).
  - The sine table is computed rather than read.
  - The `rand()` seed and call history differ from the game's, so random spark directions differ in detail.
  - Additive blending happens in linear space.
  - The hit contact uses height = radius for sphere hit shapes.
- **Not yet implemented:**
  - pppRyjMegaBirth (201): the glow and ring particles of the sparks and the Shadow's death smoke. A translation is in progress.
  - pppDrawMdl3 (137): model layers; the model renderer FUN_1404dc760 is not decoded.
  - Loop ranges and catch-up ticks.
  - The random hit-effect pick (no Sora row uses it).
  - The 0x80 depth-sorted flag.

## Verdict / next

PROGRESS: Keyblade hits now draw the original's spark effect from the installed data through a translated particle VM. Next: MegaBirth, the Shadow death smoke, then Guard.
