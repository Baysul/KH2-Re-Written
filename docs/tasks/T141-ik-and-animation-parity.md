# T141 — IK and animation parity: aim chains, pole vectors, joint-space blends

Owner Claude, 2026-10-16, requested by burba in the project thread. Gates P15/P18/P21 (test-room scope). Prerequisites: T057, T058, T063. Build identity: starts from `main` at `1e60690` (package 0.1.2), pinned Rust 1.99.0 Windows MSVC. Source: the selected Steam install (build 15194255, manifest `source-manifest-02`), session-validated, read-only.

User report: Sora's IK looks wrong while blocking, the Master Form idle looks very wrong, and some animations end abruptly.

## Evidence (private, outside Git)

Reference: the local PS2 motion routine (OpenKH Kh2AnimEmu, pin `7a3b945`, run through a private batch adapter) evaluated on the same installed model and motion payloads; the PC executable was read with Ghidra (FUN_1401d3b80 joint solver, FUN_1401c57b0 aim/IK frame, FUN_1401ab290 Euler write-back, FUN_1401c3a30/FUN_1401c3970 pose blend, FUN_1403c6740/FUN_1403c8a40 motion controller). Notes, payloads, reports and captures: `%USERPROFILE%\KH2FinalRewrite-private\research\animation-01\`. Public tools added: `motion_audit` (constraint/flag survey and continuity of every motion set), `pose_sheet` (software contact sheets, blend comparison), `motion_eval_compare` helper-joint option.

## Findings

1. Master Form (`P_EX100_TRIF`) motions use IK level 1 aim chains (spine 65–68, arms 85/86 and 177/178) with an up-vector pole (constraint kind 4). The evaluator skipped both, leaving ~75 units of error. Now: a level-1 joint aims +X at the position target of the next joint, faces the pole, rolls by its own X channel; position constraints on the aimed joint and on the joint it points at are the aim's target; the chain end with the rotation flag keeps its own rotation; two-joint chains accept a pole.
2. Expression rotation/translation channel reads use the stored channel values (the exe), not Euler angles re-derived from solved joints; flag-64 joints rewrite their channels from the solved matrix with Euler continuity (FUN_1401ab290). Two-joint reach is scaled by the joint scale.
3. Timed activation windows in the constraint table were tried against the reference and matched worse; they stay unused.
4. Base Sora guard (motion 173) and block recoil (motion 42) evaluate within 0.02 of the reference. The cause of the reported blocking look was not found; it may be the motion-switch blend below.
5. The room cross-faded motion switches by averaging skinned vertices, which collapses the body mid-blend. The exe blends joint by joint: parent-relative local matrices, translation lerp, X and Y axes lerped and re-orthonormalised (FUN_1401c3970). `blend_poses` now does this; the room blends joints (vertex average stays as the fallback for clips without joints). The blend clock starts at 2 and one tick elapses before the first shown frame (FUN_1403c8a40, FUN_1403c6740).

## Results against the reference (max joint translation error)

- Master Form clips checked first: 75 → ≤0.02. Whole player motion sets: 896 of ~930 slots ≤0.1; remaining 0.1–2.3 (one Wisdom Form clip ~30).
- All 636 installed motion sets, 8254 interpolated motions sampled every 8 key units: 7360 ≤0.1 after these fixes (89%). Still above 0.1: 894 motions, mostly enemy/boss/NPC rigs (examples: `M_EX710` non-finite, `B_EX330`, `B_EX180`, `P_HE000`, `P_BB000`, `M_EX540/550/560/580`).

## Checks

PASS `cargo fmt --all --check`; PASS clippy with warnings denied; unit tests added for the aim frame, Euler continuity and the blend. See the changelog for the final test/build status. NOT RUN: interactive playtest; comparison with captures of the installed game; the PC executable's own evaluator output (the reference is the PS2 routine).

## Uncertainty and not done

- The reference is the PS2 routine, assumed equivalent to the PC code.
- Not ported: the exe's special blend for IK-flagged joints, the new motion held at its start phase during a blend, walk/run phase sync, the first-tick time quirk, loop at FrameReturn. Out-of-order constraint sources read the previous tick in the exe; the evaluator is stateless.

## Verdict / next

PROGRESS: Master Form pose evaluation matches the reference; joint-space blends replace vertex averaging. Next: the remaining >0.1 rigs (start with `P_HE000` helper 215 ordering), the exe's chain-at-top order, controller timing.
