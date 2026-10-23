# T221 — The party's battle styles: leash, keep tests, picks and Party Attack's spacing

Owner Claude, 2026-10-23, requested by burba in the project chat ("finish all summons, limits, and
party stuff until its 1:1"; "keep going"); follows T220, which lets Customize set the style.
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T215, T220. Build identity: branch `claude/project-thread-ww9y8z` from `main`
after T220, package 0.1.2, Rust 1.99.0 Windows MSVC, release built with `--features skate3`.
Source: the selected Steam KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`: headless
Ghidra 12.1.4 decompiles `out/w220` (the twelve keep and pick routines, the personality getters,
FUN_1401b2880, FUN_1401b53b0, constants `consts.txt`), `out/p1/1401b8910`, `out/p4/1401b41a0`,
`party-camp-spec.md` §3; runs `kh2-party-1to1/runs/st-00`, `st-42`, `st-35`, `s220-*`.

## What the executable does

FUN_1401b8910 runs every tick for a member (record +0xf4 = its style, read through vt+0x158):

- Leash: the member's flat distance to Sora over 1200 (personality +0x80) × 0.5 for Huddle, × 2
  for Party and Relentless raises flag 0x40; FUN_1401b41a0 clears it when the member stands at
  its spot behind Sora.
- Out of battle the target is cleared. In battle, the current target is kept while it is valid,
  the flag is down, no summon runs and the style's keep test (0x140723ad0) passes; otherwise the
  style's pick (0x140723b30) chooses again (a summon's pick keeps the old one).
- Keep tests: Technic drops the target for another last attacker (obj+0xf88) or beyond 1200
  (+0x6c); Target drops it for a lock-on on another enemy; Huddle keeps it within 1200 and within
  (100 + 1000) / 2 of Sora; Party keeps it within 1200 and at least 1000 (+0x88) from Sora (and
  re-picks after an action record without flag 1); Sora Attack always drops; Relentless never.
- Picks (distances to the enemy's nearest collision point): Technic and Relentless take the last
  attacker within 800 (+0x68), else the nearest enemy within 800; Target takes Sora's current
  target, else his lock-on within 800; Huddle the nearest within 800 whose surface is within 100
  (+0x84) of Sora; Party, within 1600, the one farthest from Sora (from −1.0, DAT_140623c48);
  Sora Attack none.
- Party Attack in battle doubles the follow thresholds toward Sora (FUN_1401b41a0) and the
  formation distance (FUN_1401b2450 / FUN_1401b4bd0).

## What the room does now

- `player::party_style` holds the leash, the six keep tests and picks, and the per-member state;
  the room builds the candidates (the Shadow, the gallery enemies, a tour's fixed target) with
  keys that stay theirs, records each member's last attacker, and maps Sora's selection and lock
  to those keys. While the leash flag stands the member follows Sora.
- Members choose no target out of battle (the battle flag's 45-tick delay applies).
- Party Attack doubles the follow thresholds and the formation distance in battle.
- `--party-styles D:G` sets the members' styles as tour setup; `party_target` events log each
  change of target or leash flag.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new tests for every style's keep test and pick and the leash) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--autoplay --party-styles 0:0` (private `runs/st-00`) | exit 0; both members take the Shadow in battle and drop it as it goes; Donald 28 hits, Goofy 4 |
| Unattended `--autoplay --party-styles 4:2` (private `runs/st-42`) | exit 0; Donald (Sora Attack) never takes a target (no hits); Goofy (Huddle) takes the Shadow only near Sora and the 600 leash pulls him back (flag raised at 157, cleared on settling) |
| Unattended `--autoplay --party-styles 3:5` (private `runs/st-35`) | exit 0; both fight the lone Shadow (Party re-picks it each tick as nothing else stands in reach) |
| Unattended `--party-hit-tour`, `--party-items-tour` (private `runs/s220-*`) | exit 0 |

## Uncertainty

- [I] The flag 0x40's reader was not traced: the member follows Sora while it stands.
- [U] Party's re-pick after an action record without flag 1 is left out (the record flags are not
  ported).
- [U] The formation distance itself is still a test-room value (DAT_14071d760 is filled at run
  time); only its doubling is the exe's.
- The room has one Shadow and few gallery enemies, so the styles' choices between several enemies
  are covered by unit tests, not by runs. Not compared with the installed game.

## Verdict

PARTIAL. The style rules run from the executable; nothing is compared with the original game.

## Next

The Drive form effects the form-effects research lists (special bone codes, form hit sparks,
Wisdom's bullet, the second Keyblade's trail, the drive afterimage's zoom).
