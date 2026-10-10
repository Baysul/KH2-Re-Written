# T057 — Sora IK, constraints, field/battle modes and root motion

Owner Claude, 2026-10-08. Gates P15/P18/P21 (test-room scope). Starts from T056 (`accfa30`). User report with a reference image: Sora's IK and animations looked wrong; the reference shows field-mode Sora walking upright with relaxed arms and no Keyblade. The user also asked where to launch the game.

## Prerequisites / source

Same selected Steam source and private manifest as T056 (quarantined; full rehash at every launch). Public references: OpenKH `docs/kh2/file/anb/motion.md` (pin `7a3b945c538d32c6a285128c98aefba093f52ceb`) and kenjiuno's msetDoc README (gitlab.com/kenjiuno/msetDoc, commit `420935653bd95da02fd9a58a2287c8a5396c76e5`). Both describe table layouts; neither documents the solver. OpenKH's accurate playback route (`OpenKh.Kh2AnimEmu`) emulates translated PS2 code and is not used. The Steam executable's live code differs from the file (T045–T050), so no native solver was read.

## Findings

1. A new private probe (`motion_ik_probe`) dumps Sora's skeleton and the IK tables of the twelve selected motions. Every motion carries an IK skeleton (helpers 228+), a joint processing order with flags, and constraints. Idle uses 11 helpers and 6 constraints; run/fall/attacks use 39–62 helpers, 43–103 constraints, 6–14 limiters and 18 expressions.
2. Legs are IK chains: joint 11 (order flag ik=2) → 12 ("calculated") → 13 (translation+rotation flags) reaching a Position constraint target; the hip joint 10 is aimed at a pole helper by a Direction constraint. Arms use 78 → 79 → 80 and 170 → 171 → 172 (ik=3, translation flag only). Bones extend along local +X.
3. The T053 basic public FK consumer ignores all of this, which left legs/arms in bind-like orientations (the user's report). It also: leaves a channel at its bind value before the first key; extrapolates past the last key toward the first key; and uses the left key's in- and out-tangents for Hermite instead of the left out-tangent and right in-tangent.
4. Root-position curves index the combined forward+inverse curve list (e.g. A300's translate-Z root curve is inverse curve 29). In A300/A301/A304 the pelvis moves forward together with the root curve (A300: root Z 0 → 136, pelvis Z −5 → 130), so the lunge is root motion.
5. The battle-mode clips selected in T056 (weapon drawn) are not the field-mode look in the reference. The public slot chooser with battle=false, has_weapon=false gives field idle/walk/run slots 2/6/10.

## Implementation

- `package_reader::motion_eval` (new): evaluates bones plus IK helpers; inverse curves animate helpers; processes joints in the motion's joint order; applies Position, Orientation, Direction (+X aim), Position Limit (box or distance limiter) and Rotation Limit (Euler box) constraints with activations; solves two-joint IK chains analytically; evaluates expression trees (arithmetic, comparisons, condition lists, channel reads); samples curves with first/last holds, repeat/zero/additive cycles and standard cubic Hermite (out/in tangents scaled by key span). `Skinning::sample_evaluated` skins the result. The basic FK path is unchanged (`--basic-fk` keeps it selectable).
- `player_sim`: field/battle modes (attacking draws the weapon; 6 s without attacking puts it away), attack root motion applied along facing with wall/ledge checks. Twelve tests.
- `test_room`: precomputes evaluated poses with root translation removed from the rendered pose, field idle/walk/run clips, Keyblade shown only in battle mode, and a no-argument launch that reads `%LOCALAPPDATA%/KH2FinalRewrite/test-room-launch.json` (`steam_root`, `manifest`, `config`, optional `options`). The console stays open on errors when double-clicked.

Unsupported constraint types (path, up-vector, two points, scale, camera, int) are counted, not approximated; none occur in the twelve selected motions. Limiter damping, scale channels and root rotation are not applied.

## Evidence (private, outside Git)

`%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/`: `ik-research-01/motion-ids.json`, `ik-tables.json` (tables), `ik-tables-02.json` (adds evaluated root/pelvis samples); autoplay runs `autoplay-12` (evaluator, battle mode) and `autoplay-13` (field/battle modes and root motion), each with log and captures. Admission totals for one session: 24806 constraint applications, 2572 IK chain solves, 5184 expression evaluations, 0 unsupported constraints across all precomputed frames.

Inspected captures: field walk/run from behind match the reference's upright posture, relaxed arms and back X-belt; field idle from the front is upright with relaxed arms; battle idle from the front shows the two-handed ready stance with feet on the ground; attacks lunge forward (player moved 312 units across the three-hit combo) without the pose snapping back; air combo keeps the Keyblade in the right hand.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (147 tests); PASS build; PASS 180-frame smoke; PASS autoplay-13; PASS release `test_room.exe` launched with no arguments (launch file) and exited after a bounded run. NOT RUN: interactive playtest by the user; any side-by-side comparison with the installed game.

## Uncertainty

The solver rules (constraint application order, +X aim axis, two-joint bend about local Z with the bind sign, expression list/condition semantics, tangent scaling, activation defaults) are inferences from the data and public layouts, checked only visually against the user's reference image. The field/battle switch and its timeout are test-room choices. No installed-game equivalence is claimed.

## Verdict / next

PROGRESS: Sora's poses now use the motions' own IK and constraint data, field mode matches the reference posture, and attacks carry root motion. Next: the user's playtest feedback, root rotation for spin finishers, limiter damping, hit detection with a target, and a pose comparison against a captured reference frame once one is available.
