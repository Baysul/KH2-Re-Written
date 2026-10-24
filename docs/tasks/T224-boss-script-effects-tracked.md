# T224 — The boss's script effects on the tracked effect path

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep making this until
there's nothing left to do", then "keep going"), from T223's finding. Gate: enemy AI and VFX
(P19/P20 work; nothing here is inferred as a gate pass). Prerequisites: T206, T207 (the summon's
tracked effects), T223. Build identity: branch `feature/boss-script-fx` cut from T223's branch;
package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build 15194255, validated at
every run; the install was only read.

Private evidence (never in Git): the script listing `research/sephiroth-01/n_hb630.lst.txt`
(Shadow Flare at 6334..6874, the effect-position helper at 393) and the runs
`kh2-boss-out/cmp6/`, `cmp7/`. No Ghidra MCP was used; no new decompiles were needed.

## What the executable does

The object effects of a script (FUN_1403cc650) are effect instances the script keeps handles to:
`effect_is_active`, `effect_pos`, `effect_set_pos` / `_dir` / `_scale`, `effect_loop_end`,
`effect_kill` act on them, and each plays on the container of the object that made it.
Sephiroth's Shadow Flare makes a B_EX360 object per orb, gives it attack 1636 (radius 50) and
moves it with its effect in a loop that runs only while the effect is active; another attack
(1354, radius 100) follows an effect's position the same way.

The room played the boss's script effects fire-and-forget on his own container (`drive_fx`,
`ActorElement`): no handles, so `effect_is_active` answered no, `effect_pos` had nothing, and the
commands were dropped. Shadow Flare's loop ended on its first tick.

## Steps

1. `player::magic::Anchor::Actor`: a battle actor by gallery id; `spell_fx` resolves it from the
   actor's root and heading, as `drive_fx`'s actor anchor does.
2. The script brain's effect bridge takes its anchor and key range from its host (the summon's
   unchanged); a drawn brain no longer also queues the untracked request.
3. `enemy_battle`: a script brain's effects go on the tracked path. Before its tick it gets the
   effects' state after the room's last effect update (keys `2 << 56 | actor << 32 | handle`);
   after it, its calls play through `spell_fx`, each on the PAX of the object that made it
   (loaded on first use from the object entry's model, `obj/<model>.a.us`).

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper,
`python tools/progress.py` and its tests, and tours against the install.

## Results

- fmt, clippy (no warnings), workspace tests (all pass; one test ignored as on `main`),
  workspace build: PASS (one new test: an actor anchor follows that actor's root). Release
  helper: PASS.
- Tick by tick around his first Shadow Flare (`cmp7/fine`): the B_EX360 orbs live while their
  effects run (up to 8 at once from tick 489) and leave as each effect ends or the orb is hit;
  Sora's combo struck two orbs.
- Sephiroth, 6000 ticks (`cmp7/meteor`, HP 850 from tick 300; `cmp7/death`, HP 30): exit 0; at
  most 7 orbs at once, none left after tick 1590; in the death run he died, the mission cleared
  and the clear overlays ran. Hits on Sora and the party in the Meteor run against T223's build
  (`cmp6/meteor`): attacks 1635 and 1637 (Shadow Flare's) 12 and 27 against none, 1182 22
  against 5; the others within run-to-run variation. The boss's effects now run on the tracked
  path (275 tracked effect starts against 52).
- Tours against T223's release (`cmp7/`, the generated arena): Limit Form and summon identical,
  party-limit identical but for the exit tick. All exit 0.

## Uncertainty

- Nothing was compared with the original game, including how long the orbs fly and how often
  they hit.
- [I] The actor anchor uses the actor's root and heading only, as `drive_fx` did: bound
  elements on his bones (codes below 0x4000) take his root, and the body-centre codes take
  Sora's heights.
- The boss's motion-triggered effects (frame trigger 1) and his death effect stay on the
  untracked path; the script never names them.

## Verdict and next task

PARTIAL. The boss's script effects follow the script's handles and commands; nothing is claimed
1:1 by observation.

Next: the actor's bones for bound effects, map bodies for ordinary enemies under the script brain,
and a side-by-side capture against the installed game.
