# T216 — Lens-flare and world-matrix effect programs; Sora's Keyblade in summon limits

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep working on it until
there's nothing left to do", "keep going"), handed over by the project's coordinator session.
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T150 (effect VM programs, census), T210 (Limit Form on the script engine).
Build identity: branch `claude/project-thread-ww9y8z` after T215, package 0.1.2, Rust 1.99.0
Windows MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build,
validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`
(headless Ghidra 12.1.4 on a read-only copy of the analysed project: program table records and
constants `out/e1`, decompiles `out/e2`, `out/e4`, `out/e7`, `out/e9`); the effect census
`kh2-party-1to1/census.tsv` (before) and `census2.tsv` (after), the element dump
`kh2-party-1to1/lens38.txt`, runs `kh2-party-1to1/runs/lft7`, `runs/ff1`, `runs/s9`..`runs/s11`;
the capture spec `capture-weapon-spec.md` with decompiles `out/k1`..`out/k14`.

## What the executable does

- The effect census found Limit Form's element 38 (its combo finisher's effect, started at the
  finisher's hit) hidden by programs 188, 189 and 191, and Final Form's elements 44..47 by
  program 99. An unsupported program hides the whole effect.
- Program table 0x140779a00 (0x50-byte records; update +8, draw +0x18, create +0x38, restart
  +0x40): 188 pppKeLnsLpSft (FUN_1404d5c20, create/restart FUN_1404d5ce0), 189 pppKeLnsLp
  (FUN_1404d6c30, draw FUN_1404d6d10, create FUN_1404d6cb0, restart FUN_1404d6cf0), 191
  pppKeLnsClm (FUN_1404d63a0, draw FUN_1404d60e0, create FUN_1404d60b0), 99
  pppRyjMatrixWorldXYZ (FUN_1404c52e0, create FUN_1404c5460).
- pppKeLnsLp keeps a lens block: its draw (FUN_1404e26c0) finds the line from the light to the
  screen centre (screen points for a point light, the view position for a light at infinity),
  a frame along it with a step of depth / 512, and a strength from the view angle (falling to 0
  at a third of a turn), an angle limit and a fade length; its create puts the instance on draw
  layer 2. pppKeLnsLpSft moves the angle limit and fade length by a keyed rate.
- pppKeLnsClm (FUN_1404e1b30) draws a column of flip-book sprites (FUN_1404ae220) along the
  line: four sprites placed by a 16-bit LCG (s · 0x80d + 7, / 65535, FUN_1404e1f40), each
  stepping its flip-book by 0x200 a draw, sized by its place and alpha `(place · 0.75 + 0.25) ·
  fade · alpha · strength`, the fade from the light's screen distance past the fade start.
- pppRyjMatrixWorldXYZ: cur = Rxyz(angles) at the position, then the inverse of the instance's
  placement now · cur · its placement at creation (FUN_1404be820 = Rz·Ry·Rx, FUN_14013f3a0 the
  inverse), axes scaled: the part stays in the world while the instance moves.

### Sora's Keyblade in the summons' limits

- A capture (`FUN_1403c6cc0` -> `FUN_1403c64a0`) names the parts `"%03d0"` from the objentry
  (Sora's body "0010") and, for a player-type victim, raises the last digit once per weapon object
  (+0xd60 / +0xd68: "0011", "0012"); `FUN_1403c65e0` starts the first loaded entry of that name at
  frame 0. The weapon stays attached where it was (+0x6a0 owner, +0x6a4 bone, +0x6a6 flags,
  `FUN_1403d9fe0`); `FUN_1403ccb90` places it at its pose through its bone (a real bone with the
  flip diag(1, -1, -1)).
- Only Stitch's limit moves it: `FUN_1401c9d60` re-attaches each weapon at bone code 0x4000,
  Sora's own frame with no bone and no flip (`FUN_1403b50f0`), and puts it in camera space with
  Stitch's second matrix; `FUN_1401c7e00` returns it to the hand. Chicken Little
  (`FUN_1401cb6c0`) and the Genie / Peter Pan command (`FUN_1403de460`) make no weapon calls;
  Sora's +0x120 bit 8 there is the no-damage flag, not a hide flag (T207's "Sora is hidden" for
  Chicken Little's limit has no support).

## What the room does now

- `effect::lens` translates the three lens programs; `extra` adds pppRyjMatrixWorldXYZ.
- The census goes from 15 unsupported kinds to 11; none left belongs to a form, a party member,
  a summon or a limit.
- During Stitch's limit Sora's Keyblade is drawn in his root's space (no hand bone): the captured
  "0011" pose's centre then sits by his hand joint (tick 4200: (-7, 127, 25) against the hand at
  (-5, 125, 16)); through the hand it had hung at his hip ((-30, 33, -25)). The other summons'
  limits keep it on the hand bone with the captured pose, as the executable does.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new: the rigid inverse of pppRyjMatrixWorldXYZ) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Effect census (private `census2.tsv`) | 11 unsupported kinds (15 before); 99, 188, 189, 191 gone |
| Unattended `--limit-form-tour` (private `runs/lft7`) | exit 0; element 38 starts at the finisher (tick 329) with no unsupported program and draws its flare sprites |
| Unattended `--summon-tour` (private `runs/s9` before, `runs/s10`, `runs/s11` after) | exit 0; Stitch's limit draws the Keyblade at Sora's hand (logged positions above, `runs/s11`); the other limits unchanged |
| Unattended `--magic-tour --magic-form 5` (private `runs/ff1`) | exit 0; Final Form runs with no unsupported program; no run plays elements 44..47 |

## Uncertainty

- [I] pppRyjMatrixWorldXYZ takes the instance's placement from its matrix (axes without their
  lengths) instead of turning its radian angles through the sine table.
- [I] The sprite packet's flag bits (0x30) and mode word of FUN_1404ae220's call are not
  modelled; the sprites draw with their own blend byte.
- [I] The global pause flags (`DAT_142b0efc0`, instance +0xc1) are taken as clear, as elsewhere.
- [I] The room finds the capture's parts by entry position and set variant, not by the name
  scan; the same entries only if the summons' sets keep that layout (not checked). A missing
  "0011" leaves the previous clip's Keyblade pose shown (the executable clears the motion).
- Zantetsu Counter (Limit Form's record 0x27 out of the guard recoil) is not exercised in a run.
- Not compared: the flare's look against the installed game; Final Form's elements 44..47 are
  only stepped by the census.

## Verdict

PARTIAL. The four programs are translated and the effects they hid now run, and Sora's Keyblade
in Stitch's limit is placed as the executable places it; nothing is compared with the original
game's frames.

## Next task

A controlled frame comparison of Limit Form's finisher flare; the camp's Customize and
Abilities screens for the party (T215's open item).
