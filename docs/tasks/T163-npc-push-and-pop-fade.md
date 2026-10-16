# T163 — Twilight Town townspeople: pop-in and pop-out fade, and the push

Owner Claude, 2026-10-16, requested by burba in the project chat (GitHub issue
#24). Gate: development of the test room only; no final gameplay acceptance is
inferred. Prerequisites: T097 (Twilight Town). Build identity: `main` at
`e27f7f0`, package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam
KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/` — the
routines and constants are in `playable-room-20261008/tt-world-01/look-02/`
(`look-spec.md` §2, §3) and `npc-01/npc-spec.md` §4; unattended runs `npc1`..`npc3`
under `levelup-20261016/`.

## What the executable does

- Push (`FUN_1403b9090` → `FUN_14040ad70`, separation `FUN_1403c90b0`): when
  Sora's OBJ ellipsoid touches a townsperson's, the separation `d` points from
  Sora to the townsperson. A movable one (weight 500 against Sora's 100) takes
  `d × 100/600` as a one-shot displacement and Sora is moved back `d × 500/600`;
  a talking or fixed one (`FUN_1403ba7d0`) does not move and Sora is moved back
  by all of `d`.
- Pop-in: a zone-spawned (stype 2) townsperson is built when Sora is inside a
  kind-1 zone of its spawn point; its alpha factor (`sys+0xa08`) ramps 0 → 1
  linearly over ten ticks (`FUN_1403b7810`, `FUN_1403b77c0`, 10.0 at
  `0x140623bb4`).
- Pop-out (`FUN_140416270`): after five consecutive ticks outside the zone the
  factor ramps 1 → 0 over ten ticks (the NPC keeps running its AI), and the object
  is released when it reaches zero. A re-entry while it fades out waits until
  it is gone.

## Rewrite

- `npcs::push` (the ellipsoid separation and weights), `FieldNpc::{pop_in,
  pop_out, tick_fade, immovable}`; `field_npc_tick` builds zone NPCs fading in,
  starts the fade-out after the five ticks, releases the NPC at zero and applies
  the push to Sora and the townsperson each tick; `field_npc_present` sets the
  parts' alpha (blend while fading).
- Log events `npc_pop_in`, `npc_pop_out`, `npc_gone` and `npc_push`; the
  periodic state line carries each townsperson's `fade`.

## Checks

- Unit tests: the 1/6 and 5/6 split, the immovable case and no push out of
  reach; the ten-tick fade in and out and the release. fmt, clippy (warnings
  denied), tests, build and smoke PASS.
- Area 6 autoplay against the installed build (`npc3`): three zone townspeople
  popped in, one popped out and was released (`npc_pop_in`, `npc_pop_out`,
  `npc_gone`); the push never came up (Sora does not touch anyone).
- `--npc-tour` (`npc4`): Sora placed 30 units from a townsperson was pushed 79
  units back in one tick (`npc_push`), which is the formula's value for the
  overlap (a one-shot displacement); a capture shows the two apart.

## Not reproduced / uncertain

- Walls do not stop the push of either side (the exe sweeps both against the
  map).
- The pop-out waits for no event / menu (`FUN_1403abc80`); only the zone and
  the talk gate it.
- A townsperson's weight is fixed at 500 (the RTN value), not read from
  objentry +0x50; objects with weight 10000 or other immovable flags
  (`0x120` bit 2, attached, class flags) are not modelled.
- The other behaviours of issue #24 (sitting, rtn_lm, skaters) are not
  ported: they need the rtn variants' scripts.
- The alpha ramp was checked by the log and unit tests, not by a capture of a
  half-transparent townsperson, and there is no installed-game capture comparison.

## Verdict

PARTIAL. Next task: the sitting and other NPC programs.
