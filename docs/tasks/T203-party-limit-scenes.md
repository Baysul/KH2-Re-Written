# T203 — Party limits run from the second deck

Owner Claude, 2026-10-20, requested by burba in the project chat (follow-on to T202: "make
picking a summon, a Party entry and a party limit actually do their action after the menu
closes, recovered 1:1 from the exe"). Gate: development of the test room only; no final
gameplay acceptance is inferred. Prerequisites: T202 (the second deck), T186/T201 (the party
in the room), the `enemy` crate's object-script engine. Build identity: branch
`claude/project-thread-6zu7ov` from `main` at `3645961`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build,
validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/command-deck-20261020/`
(decompiles `out/d9`..`out/d37`, trap tables `out/t6`, `out/t7`, data dumps `out/dd5`..`dd7`);
script listings and lifts of the limit programs, probe output and the unattended runs in the
private `kh2-command-deck/` folder (`bdx/`, `runs/final4`).

## What the executable does

- Picking a Limit-list entry starts the limit (`FUN_1403d8b40`): all of Sora's MP goes
  (cost 0xff, MP Charge starts), the members not taking part fade out, and the limit object
  (`FUN_1403d7fa0`, 0x128 bytes, `DAT_142a24cc0`) loads `limit/<lang>/<file>.bar`
  (`FUN_1403d96d0`): the scene program (BDX type 3, trigger 22 = start, 23 = follow-up), the
  limit's own motion set (`LIMI`, keyed `"%03d0"` by objentry +7), its PAX, cameras and the
  gauge sheets. The time is the command's +0x20.
- The scene offers commands through `command_override` (`FUN_1403b2850`): record 0 takes
  the row of the Attack kind, record 1 is the reaction (Triangle) command, record 2 takes the
  Limit row (command 0x51), records 3–6 the whole deck (`FUN_1403b0f20`). The cursor keeps
  its row (`FUN_1403b11e0` follows the remembered command id).
- A command is taken only when `FUN_1403b6950` agrees: the command's +0x1f names whose
  motion windows decide (0 the user, 1 the override's target friend, 2 both), and
  `FUN_1403ba070` accepts inside a window the motion opens — range 0x14 (flag bit 17) for the
  command itself, range 0x1a (bit 21, value at +0x638) for the command whose +0x22 names it,
  or the free windows (ranges 0–2, flag bits 7/8/2 of +0xc) — and refuses under range 0x28.
- Each motion start (`FUN_1403c64a0`, also for a queued motion in `FUN_1403c6740`) gives the
  object a new token at +0x184; the scene waits on it to see a motion replaced.
- The gauge (`FUN_1403d9130`) runs down by 1.0 × the ability rate each frame while no
  participant's motion holds range 0x2d (+0x194 bit 0).
- Duck Flare keeps a flare pool (traps 7:3–7:11; `FUN_1405410e0`, 46 records of 0x50 bytes,
  free stack at +0xe60): every frame (`FUN_140541180`) overlapping flares part by half the
  push-out each (`FUN_1403c94c0` / `FUN_1403c90b0`, cylinder and sphere shapes, margin 5.0)
  and are pushed out of Sora's kind-4 shapes (his model's one sphere, radius 90, 115 up).
- Enemies are team 2 (`FUN_1403ab4a0`, `FUN_1403d0150` test +0x4dc).

## What the rewrite does

- `enemy::limit_scene::Scene` runs the program with Sora as the player stand-in and the
  friends and their weapons as engine objects; `enemy::traps::limit` holds the limit trap
  group (2:47–2:63, 2:89, 1:263), `enemy::traps::flare` the flare pool, `searcher` gains
  6:12 and 6:21, target records may live in script memory, `obj_can_decide_command` follows
  `FUN_1403b6950` (`traps::state::decides`).
- `test_room::party_limit` starts a picked limit, ticks the scene in place of Sora's own
  update, puts his and the friends' limit motions on screen, plays the file's effects, lands
  the scene's attack objects and direct strikes on the Shadow with the owner's stats, and
  logs `party_limit_start/offer/follow_up/tick/attacks/end` (`KH2_LIMIT_TRACE` adds every
  trap called).
- The command menu shows the offered commands on their rows with the override art; a taken
  row runs the limit's command and no longer opens the Limit list.
- `--party-limit-tour <dir>`: locks on, then for each Limit-list entry stands Sora before the
  Shadow with full MP (tour setup), opens the list, picks it, presses the offered command and
  the reaction button, and captures.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | FAIL: two `enemy::traps::weapon` tests (pre-existing on `main`, the module is unchanged); all others pass, including the flare pool and tour tests |
| `cargo build --workspace --locked`, release `--features skate3` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Unattended `--party-limit-tour` (private `runs/final4`) | see below |

Unattended tour, locked on to the Shadow:

| Limit | Runs to its end | Follow-ups taken | Hits on the Shadow | Notes |
| --- | --- | --- | --- | --- |
| Whirli-Goof | yes | 283 ×2, 256 ×3, 257 | 21 | PASS |
| Knocksmash | yes (early) | 339 ×4, 340 ×6 | 0 | FAIL: two faults in the lock-on branch, a target record resolving to the scene program's own handle; unlocked runs landed 22 hits |
| Comet | yes | 280 ×5, 281 | 0 | FAIL: the impact effects (elements 2–4) end on their first frame in the room's effect runner, so the 6-frame strike wait never completes |
| Duck Flare | yes | none | 21 (flares) | FAIL: 348 waits for Donald's motion 252 (range 0x1a, chain 279); after 254/255 Donald drops to slot 0 instead |
| Trinity | times out | none | 0 | BLOCKED: the `comd` button-combo sheet and the shot traps 7:28–7:30 (`FUN_140411840` projectile manager) are not ported |

Not done this task: summons (BLOCKED, a separate system); the Party command runs trigger
11 of the member's script, which Donald and Goofy do not have (no visible effect, as in the
exe); the limit gauge/combo HUD and Trinity's camera are not drawn.

## Uncertainty

- Lift argument order for script routines is not settled; the Knocksmash lock-on fault was
  not traced to a cause.
- Engine objects keep no command lockout (+0xb94); Sora's flag-only window terms are left to
  his kit.
- The participants' queued motion 0 during a limit may name a limit motion in the
  executable (`FUN_1403c8310` not traced).

## Verdict

PARTIAL. Whirli-Goof runs 1:1 in the test room as far as checked; the other four start, take
their commands and end, with the failures above.

## Next task

Trace the queued-motion and lock-on gaps (Duck Flare, Knocksmash), the effect-VM lifetime of
Comet's impact elements, then Trinity's combo sheet and shot manager.
