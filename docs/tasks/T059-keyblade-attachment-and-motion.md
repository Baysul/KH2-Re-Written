# T059 — Keyblade attachment bone and weapon motion

Owner Claude, 2026-10-09. Gates P18/P21 (test-room scope). Starts from T058 (`0f26873`). User goal: the Keyblade position should match the original.

## Prerequisites / source

Same selected Steam source and private manifest as T056–T058 (quarantined; full rehash at every launch). Public reference: OpenKH `docs/kh2/file/type/03system.md` (SKLT table layout, pin `7a3b945c538d32c6a285128c98aefba093f52ceb`).

## Findings

1. The objentry WeaponJoint byte used since T056 (25 for `W_EX010`) is not a hand. In the reference evaluator's battle idle, bone 25 sits at the hip (same position as bones 10/11, on the 0→1→2→3→10→11→23→24 chain), while the two hand chains end near bones 86 and 178. Earlier captures held the Keyblade near the waist, which only looked right while the hands were low.
2. The system archive's SKLT table names the weapon bones per character. The installed table's entry for character 1 (Sora in the public dictionary) gives primary bone 178, the end of the 166→…→172→173→…→178 arm chain.
3. The weapon's own motion set (`W_EX010.mset`, located in T058) has clips at Sora's slot numbers. Its bone 0 is animated to a small offset (≈ 3.7, −0.1, 3.6 at battle idle) and bones 3–11 (key chain) move per frame; the posed vertices are in the attachment bone's space.

## Implementation

- `package_reader::sklt` (new): SKLT reader with a synthetic-bytes test.
- `test_room`: optional `sklt_character_id` selects the SKLT primary bone as the weapon joint (objentry byte kept as the fallback and logged as `objentry_joint`); optional `weapon_motion_set` loads the weapon clips for every battle slot, evaluates them with the same evaluator at the same frame as Sora's clip, and updates the Keyblade meshes each tick in the attachment bone's space. Without the weapon set the bind-pose fallback is unchanged.

## Evidence (private, outside Git)

`%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/`: `test-room-tt08-05.json` (adds `weapon_motion_set` and `sklt_character_id: 1`), `autoplay-20` (weapon motion, objentry bone 25), `autoplay-21` (weapon motion, SKLT bone 178; log records `weapon_joint` 178 and `objentry_joint` 25). Reference positions: `anim-oracle-01/ref-02/motion-000-slot-0000.bin`.

Inspected autoplay-21 captures: battle idle from the front holds the Keyblade low in front in one hand with the chain hanging from the grip; the air jump raises the Keyblade overhead in the raised hand; the run-to-attack and attack captures keep it in the hand.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (147 tests); PASS build; PASS 180-frame smoke; PASS autoplay-21; PASS release `test_room` build. NOT RUN: interactive playtest by the user; side-by-side comparison with the installed game.

## Uncertainty

Character id 1 for Sora comes from the public dictionary. Whether the original applies the weapon motion exactly in the SKLT bone's space (rather than with an extra offset) is inferred from the weapon clips' small bone-0 offsets and the captures, not from the executable. The secondary bone (SKLT bone 2) is unused.

## Verdict / next

PROGRESS: the Keyblade rides the game's own attachment bone with its own animation. Next: the follow camera from the PC executable, then HUD gauges.
