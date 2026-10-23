# T217 — Leaving the ground: the branch of FUN_1403b9090 was the wrong way round

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do"), from the open items of T209/T213: Sephiroth stayed in the air after
Heartless Angel, so the Split Flash that follows ran mid-air and its Block counter missed. Gate:
enemy AI and physics (P19/P20 work; nothing here is inferred as a gate pass). Prerequisites: T206,
T209, T213; T207 (summons). Build identity: branch `feature/leave-ground` cut from `main` at
`c306aff`; package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build 15194255,
validated at every run; the install was only read.

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/boss-01/` (plain
`analyzeHeadless` decompiles `out/g1`..`g5`, `h3`..`h5`, table peeks `p1`..`p3`, notes
`ha-float-notes.md`), the earlier `research/playable-room-20261008/growth-01/growth-spec.md`
(§4.3, which read the branches the same way), and the runs `kh2-boss-out/g1` and `cmp3/`. No
Ghidra MCP was used.

## What the executable does

FUN_1403b9090, after the frame's move, has two branches:

- **Leave the ground.** An object in a grounded mode (mode flag bits 0 and 1 clear; the ground
  mode's object at 0x140750950 has flags 0x180) with no gravity range (`+0x18c & 0x1000020`,
  ranges 3 and 0x1e) and no hold range (bit 6), whose collision query reports no floor contact
  (`+0x744` bit 0), and without `+0x120` bit 23: the act manager's event 1, script event 0xd, the
  mode's slot +0x28 (empty for the ground mode) and the class's vt+0x50, which falls (FUN_1403b7850:
  the mode's slot +0x20, FUN_140409b00 -> FUN_1403cbc30 and mode 1).
- **Land.** Otherwise (an air mode, or a gravity / hold range), when not rising, FUN_1403ba1e0
  ("can land") holds and there is floor contact: the act manager's event 2, script event 0xe, the
  mode's slot +0x30 (the land routine for the jump modes, FUN_1403cb940) and the class's vt+0x58.

The bit `+0x744` bit 0 is in the collision query's flag word (FUN_140170b00 / FUN_140170c10): it is
set while the object has no floor contact, which is why the land branch requires it clear.

The crate had read the land branch's condition as the leave branch's: an object fell only while a
range 0x1e held it, and an object left in the air with no range stayed there. Heartless Angel
(motion 205) lifts Sephiroth by its root motion under an open range 3; its end callback goes back to
the ground mode (`obj_idle`) and to motion 4 (his fall pose, no ranges). With the right branch he
now leaves the ground there and falls.

The motion's root motion moves the object in full, y included (FUN_1403b89a0 adds `+0x690`), and
the ground mode adds a slope term only while standing on a floor polygon (FUN_1403b81d0), so
nothing else brings him down.

## Steps

1. Traced obj_idle (FUN_14042d0e0), the mode table (0x140750d30), the ground mode's slots, the
   movement integrator FUN_1403b89a0 and its terms, and FUN_1403b9090's branches.
2. Corrected `Sysobj::leave_ground`; rewrote its unit test to the executable's rule.
3. Compared unattended tours against `main`'s release build, tick by tick.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and tours against the install.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`),
  workspace build: PASS. Release helper: see the pull request.
- Sephiroth (`g1`, HP 2000): after Heartless Angel he leaves the ground at y 590 and falls to the
  floor in about 60 ticks (fall mode 0x181), lands (motion 5) and the fight goes on from there.
- Side by side with `main` (private `cmp3/`), all exit 0:
  - `--limit-form-tour`: identical tick by tick (the draw path logged three unsupported effect
    programs in this run only: it reports from drawn frames, not from the simulation).
  - `--party-limit-tour`: identical until tick 3373; 239 hits against 236.
  - `--summon-tour`: Chicken Little's `surprise` keeps its rise while its range holds and falls
    after it (at tick 270 he is still in `surprise` at y 141 where `main` already fell); his shots
    hit 40 times against 33.

## Uncertainty

- Nothing was compared with the original game. In particular the moment Sephiroth comes down, and
  that Split Flash's motion plays while he falls and is cut by the landing, follow from the order
  of the act update and the physics step as translated, not from a capture.
- "No floor contact" is the room's floor test within one unit; the executable's collision query
  (FUN_140170c10) is not translated. The collision-enabled test (FUN_1403ba940), event 0xd, the act
  manager's event 1 and the velocity the leave branch keeps are not carried out.

## Verdict and next task

PARTIAL. The leave-ground rule follows the executable; nothing is claimed 1:1 by observation.

Next: the land branch for grounded objects under a range (land with the class's vt+0x58), the
collision query FUN_140170c10, and a side-by-side capture against the installed game.
