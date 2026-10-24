# T223 — Drive form effects: Keyblade anchors, form hit sparks, Wisdom's bullet, the second trail

Owner Claude, 2026-10-23, requested by burba in the project chat ("finish all summons, limits, and
party stuff until its 1:1"; "keep going"), with the coordinator session's item "Drive form
particles and afterimage (Valor/Wisdom/Limit/Master/Final/Anti), from the form's own effect data".
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T108, T150, T221. Build identity: branch `claude/project-thread-ww9y8z` from `main`
at `ad92e12`, package 0.1.2, Rust 1.99.0 Windows MSVC, release built with `--features skate3`.
Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`:
`form-effects-spec.md` with its headless Ghidra outputs `out/f1`..`out/f12` (every effect-start
caller, FUN_1403b50f0's bone codes, the weapon tick FUN_1403efa60, the form PAX category census,
the afterimage control block); runs `kh2-party-1to1/runs/fx222a`, `fx222-f2`, `fx222-f4`,
`fx222-f5`.

## What the executable does

- No form starts a looping or persistent element from code: the form effects come from motion
  triggers, the drive and revert ids, and the weapon objects (form-effects spec §1).
- FUN_1403b50f0 resolves bone codes 0x4003 / 0x4004 to weapon slot 0 / 1's blade tip, 0x4005 /
  0x4006 to the slot's grip, 0x4007 / 0x4008 to the blade's middle. Wisdom's shots (MAGF 25..43),
  Master's arrival (TRIF 100 / 102), Final's (ULTF 101 / 102) and many of Limit's and Final's
  attack elements bind to them.
- A hit spark's container is the attack's owner: windows in a weapon motion use the Keyblade's
  PAX, windows in Sora's own motion set use the shown object's PAX (Anti 13..16, Final 41, Limit
  46, base 14 / 42 / 43; Master's 1100 / 1101 find no element and show nothing).
- Wisdom's bullet is MAGF element 20, unbound, on the bullet's position and direction each tick,
  faded over 8 frames when its life runs out and killed by a hit; it starts at the shot element's
  first position, the Keyblade tip.
- Every weapon object runs the trail (FUN_1403efa60): slot 1's Keyblade trails on its own
  motion's range-12 windows with its own element 10 (Final's floating blades in jumps, falls and
  glides; Master's left blade while idle, walking, running).
- The drive's afterimage draws the previous frame at (516/512, 420/416) (cx = cy = 2).

## What the room does now

- Bound effects on 0x4003..0x4008 follow each slot's grip, tip or blade middle (the grip is the
  hand joint holding the weapon, the tip its farthest vertex).
- Hit sparks of Sora's own-mset windows come from the shown kit's PAX; a form with no matching
  element shows none.
- Wisdom's bullets start at the Keyblade tip and carry MAGF element 20 until they hit (killed) or
  run out (8-frame fade).
- Slot 1's Keyblade loads its own PAX and trails on its own windows (owner 0xf8), whether or not
  slot 0 is drawn.
- The drive afterimage uses (516/512, 420/416); SeqBlur keeps (514/512, 418/416).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new test for the weapon codes) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--magic-form 2 --magic-tour` (private `runs/fx222-f2`) | exit 0; shot elements 25..27 on 0x4003, twelve bullet elements 20 |
| Unattended `--magic-form 5 --magic-tour` (private `runs/fx222-f5`) | exit 0; arrival 101 / 102 on 0x4005 / 0x4006, 52..57 on both grips; slot 1's trail starts (tick 366); captures show the two floating Keyblades' trails |
| Unattended `--magic-form 4 --magic-tour` (private `runs/fx222-f4`) | exit 0; arrival 100 / 102 on both grips; the left Keyblade's trail while idle (tick 176) |
| Unattended `--autoplay` (private `runs/fx222a`) | exit 0; Limit Form's 21 (0x4003) and 33 (0x4005) on the blade; no missing spark elements |

## Uncertainty

- [I] The slot matrix is the hand joint and the blade length its farthest vertex (FUN_1403d7600
  and the returned length were not read); 0x4001 / 0x4009 (shape centres) still sit on the
  object.
- [I] Slot 1's archive is the slot-1 Keyblade item's model.
- Not built: the refraction programs 145 / 160 (Quick Run's haze and four form elements), the
  Keyblade's reappear element 0 with its white flash, the arrival replay on an area load in a form,
  slot 1's own hit-spark container. Not compared with the installed game.

## Verdict

PARTIAL. The anchors, containers, bullet visual, second trail and zoom follow the executable;
nothing is compared with the original game.

## Next

A tour where an enemy attacks Donald and Goofy in a form-free fight is already covered by
`--party-hit-tour` (T186); the refraction draw remains the largest open effect item.
