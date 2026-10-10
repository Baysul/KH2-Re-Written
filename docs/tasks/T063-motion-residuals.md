# T063 — Closing the remaining motion residuals

Owner Claude, 2026-10-09. Gates P15/P21 (test-room scope). Starts from T062 (`9e72997`). User goal: 1:1 animation parity.

## Prerequisites / source

- Same selected source and private payloads as T058: Sora's model and twelve exported clips.
- The local reference is the recompiled PS2 motion evaluator (`anim-oracle-01`).
- The installed PC executable was read with Ghidra: expression evaluator FUN_1401d9950 (private outputs `ghidra-kh2-a1/expr-01`, `expr-02`).
- Recovered PS2 rules are in the private notes from T058.

## Findings

1. **Constraint order.** The reference collects a joint's constraints, then applies them in a fixed order: position sources, then the position limit (opt1_fk_translation 0x12bb34), then rotation. We applied them in table order. In the run cycle, a foot target's ground limit (y ≥ 15) therefore ran before its position source and was overwritten. The foot then went 0.6 units through the floor and bent the knee chain.
2. **World scale is inherited.** tmpb[j] = tmpb[p] × S[j] (0x129cf8), and a child's translation goes through the parent's scaled matrix. A300's strap bone 58 has an initial-pose scale of 0.972. Its child sat 0.155 units off.
3. **Expression operators in the PC evaluator** (jump table at 0x1401da2ec):
   - SIN/COS/TAN take degrees. ASIN/ACOS/ATAN return degrees, with ±90 and 0/180 clamps outside [−1, 1].
   - SQRT takes |x|. AV is the mean of a list. CTR_DIST is the world distance between two joints. MOD is integer modulo.
   - LIST/ELEMENT_NAME evaluate to 0. FCURVE_SCAL* read the scale.

   A301's toe orientation comes from an expression using ASIN. Treating its result as radians turned the toe by up to 0.98.

## Implementation

`package_reader::motion_eval`:
- Per-joint constraints are applied in the reference order.
- `World::compose` inherits scale and scales child translations.
- The expression operators are translated from FUN_1401d9950, including CTR_DIST and AV.
- Expression channels read scale.

## Evidence (private, outside Git)

Comparison of all twelve clips against the reference, using `motion_eval_compare` on `anim-oracle-01/ref-02`. The comparison script is private.

| Metric | Before (T058) | After |
|---|---|---|
| Max translation error | idle 0.032, walk 0.021, run 0.81, jump 0.030, fall 0.032, land 0.023, A300 0.85, A301 1.67, A304 0.044, air combo 0.031/0.027/0.036 | idle 0.032, walk 0.021, run 0.016, jump 0.030, fall 0.032, land 0.023, A300 0.023, A301 0.022, A304 0.044, air combo 0.031/0.027/0.036 |
| Max rotation-matrix element error (every 4th tick) | up to 0.98 (A301 toe) | ≤ 0.0021 for every clip |

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (159 tests); PASS build; PASS 180-frame smoke; PASS release `test_room` build. NOT RUN: interactive playtest; comparison against frames captured from the installed game.

## Uncertainty

- The reference is the PS2 evaluator. The PC evaluator was read only for expression operators.
- The remaining sub-0.05 differences are float-level (f32 operation order, sin/cos implementations).
- FUNC_AT_FRAME expression nodes and the constrained two-joint path are still not translated; the twelve clips do not need them.

## Verdict / next

PROGRESS: every clip used by the test room now matches the reference to within 0.05 units and 0.002 in rotation. Next: the open HUD/menu items, then enemies and hits.
