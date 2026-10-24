# T223 — A leaving object is removed when its alpha reaches 0

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do", then "keep going"), from the Sephiroth open item "B_EX360 objects
never removed". Gate: enemy AI (P19/P20 work; nothing here is inferred as a gate pass).
Prerequisites: T206, T209. Build identity: branch `feature/leave-removal` cut from T222's branch (the map hit record, #133);
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build 15194255, validated at
every run; the install was only read.

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/boss-01/` (plain
`analyzeHeadless` outputs `out/n3`..`n9`), the script listings
`research/sephiroth-01/n_hb630.lst.txt` and `b_ex360.lst.txt`, and the runs
`kh2-boss-out/g7`, `cmp4/` and `cmp6/`. No Ghidra MCP was used.

## What the executable does

- `obj_leave` (1:28, FUN_14042c810) starts an alpha fade to 0 unless the object is already
  leaving, and sets `+0x9b8` bit 4.
- FUN_1403bfd30, the per-object update, steps the fades (FUN_1403cc730) and then, when the alpha
  (`+0xa08`) is 0 and bit 4 is set, calls the class's vt+0x38. For the base class that is
  FUN_1403b2e10: it frees the object's script and motion and sets `+0x120` bit 28, and the next
  update destroys the object (vt+0x40 / vt+0x48). The boss class's entry is FUN_1403f8ad0 ->
  FUN_1403d36d0 and the field class's FUN_1404193d0; neither is traced further.
- Sephiroth's Shadow Flare makes B_EX360 objects (entry 2322) with `sysobj_appear`, holds each on
  its effect while both exist, then calls `obj_set_flag(3)` and `obj_leave` on it (subroutine at
  6334). B_EX360's own script never leaves: its main trigger halts and its init and damage
  triggers only set a unit argument.

The crate set the leaving flag and the fade but never removed the object, so every B_EX360 stayed
in the object list (`sysobj_is_exist` still true, its script still run): 32 of them by the end of a
6000-tick run (`cmp4/meteor`).

## Steps

1. Traced the leave trap, the per-object update and the classes' vt+0x38 (`out/n3`..`n9`), and
   listed B_EX360's script.
2. `Sysobj::end_tick`: after the fades, a leaving object whose alpha is 0 is gone.
3. Unit test; tours against the install.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and tours against the install.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`; one new
  test), workspace build: PASS. Release helper: PASS.
- Sephiroth (`cmp6/meteor`, HP 850 from tick 300, 6000 ticks; `cmp6/death`, HP 30): exit 0; no
  made object remains in any 30-tick sample (32 by the end on the earlier build, `cmp4/meteor`);
  Meteor ran; in the death run he died, the mission cleared and the clear overlays ran.
- Tick by tick around his first Shadow Flare (`cmp6/fine`): each B_EX360 is made and removed on
  the same tick. Its holding loop ends at once because `effect_is_active` answers no for the
  boss's effects (they are played untracked), so the earlier build only kept them listed.
- Tours against T221's release (`cmp6/`, the generated arena): party-limit identical; summon
  identical but for the exit tick; Limit Form identical but for a 1-tick shift of the opening
  form change (both vary run to run). All exit 0.

## Uncertainty

- Nothing was compared with the original game.
- vt+0x38's other work is not carried out: the script instance and motion are dropped with the
  object, not freed by FUN_1403e1a00 / FUN_1403c7300; the boss wrapper's battle work in
  FUN_1403d36d0 is not traced. The other removal condition in FUN_1403bfd30 (`+0xaac` equal to
  0) is not read.
- Found, not fixed here: the boss's script effects are played untracked, so `effect_is_active`
  answers no and their position, direction, loop-end and kill commands are dropped. Shadow
  Flare's attack (1636) lives in a loop that runs only while its effect is active, so it ends on
  its first tick and cannot hit. This is the next task.
- The crate steps the fades before the move where the executable steps them after it; the
  removal tick is the same.

## Verdict and next task

PARTIAL. Leaving objects are removed as the executable removes them; nothing is claimed 1:1 by
observation.

Next: the boss's script effects on the tracked effect path (handles, commands, liveness, bone
binding), so Shadow Flare's loop runs; then map bodies for ordinary enemies under the script
brain, and a side-by-side capture against the installed game.
