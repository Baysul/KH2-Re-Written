# T206 — Summons, summon limits and party follow-ups

Owner Claude, 2026-10-23, requested by burba in the project chat ("finish all summons, limits,
and party stuff until its 1:1 using ghidra"), handed over by the project's coordinator session.
Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T202–T204 (second deck, party limits, summons), T186 (party hit, KO, items),
T113 (Limit Form), the `enemy` crate's object-script engine. Build identity: branch
`claude/project-thread-ww9y8z` from `main` at `1f3b241`, package 0.1.2, Rust 1.99.0 Windows
MSVC, release built with `--features skate3`. Source: the selected Steam KH2FM build, validated
each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`
(headless Ghidra 12.1.4 on a read-only copy of the analysed project: decompiles `out/d1`..`out/d14`,
cross references `out/x1`, `out/x2`, call-site dumps, data dumps `out/dd1`..`out/dd6`); script
listings in the private `kh2-command-deck/bdx`; unattended runs in the private
`kh2-party-1to1/runs/s1`..`s6`.

## What the executable does

### Magic threads (trap 2:20)

- `magic_start_thread` (2:20, `FUN_140432cd0` -> `FUN_1403c5ec0`) starts a new thread of the
  *running program* (`DAT_142a25048` is the thread being run, or its parent while a child runs;
  `FUN_1403e1c80`) at word `args[0]` with one argument (`FUN_1403e1970`, count 1) and the
  caller's owner (+0x58 / +0x60). A 16-byte holder (`FUN_1403c4e90`) puts it on the magic list
  (`DAT_142a23a70` / `DAT_142a23a78`); the script gets the holder.
- The list's task `FUN_1403c55f0` (created by `FUN_1403c5a90` with priority 0x5014: after the
  objects' 0x4e20, before the limit's 0x5078) runs each thread as a root thread
  (`FUN_1403e1c80(thread, 0)`) unless `DAT_142a11400` bit 5 holds; a thread that has ended with
  no child left is unlinked and freed (`FUN_1403c5030`). The next holder is read before a thread
  runs.
- There is no separate global "magic program": Chicken Little's shot (his script P_EX350, word
  676, a 288-byte shot record as argument) is a thread of his own program. Donald's script holds
  the same helper (L4802) but nothing calls it.

### Chicken Little's limit

- `FUN_1401cb6c0` (10:3): Sora's +0x120 bit 8 and +0x124 bit 14, Chicken Little's +0x120 bit 8,
  Chicken Little carried at Sora's origin (`FUN_1403d9fe0` bone 0x4000, flags 4), Sora in physics
  mode 0x3e (`FUN_1403b4da0`; mode object 0x140750d08, word 0x900: entry, exit and step slots
  empty, motion slot the ground idle picker `FUN_140422520`, fall/land slots to mode 0x3f).
  `FUN_1401cb180` undoes it.
- Each limit frame `FUN_1401cb050` runs `FUN_1401cb830` and turns Sora (`FUN_1403b6420`) by the
  view matrix's +0x10 / +0x30 words (`FUN_14019ce40(1)`).

### Stitch's limit

- `FUN_1401c9d60` (10:3 for Stitch): the target list cleared (`FUN_1401cd8e0`), the action ended,
  the screen point `DAT_140724320` (−218, 55, 1024), motion 0, Sora captures Stitch's motion
  0xfe (`FUN_1403c6cc0`), Sora and his Keyblades go into camera space with Stitch's second
  matrix (+0x748, `FUN_1403cbfc0`; `FUN_1401cad30`: the point `DAT_140724330` (185, 125, 1024)
  × 0.503644, unturned, w column (0, −70, 0, 1024)), Sora's shadow off (+0x124 bit 9, the old
  state kept), Sora in physics mode 0x3c (object 0x140750ce8, word 0x900, every slot empty;
  fall/land to mode 1), the friend callback `FUN_1401c7bf0` (+0xfa0, `FUN_1401b7d20`) and the
  deck sheet mode 4 (`FUN_1403db140`). `FUN_1404060e0` gives mode 4 two rows from 0x1405cc470
  (0x20e selector 3, 0x20d selector 4) or 0x1405cc468. `FUN_1401c7e00` undoes it.
- `friend_call` (7:26, `FUN_1401b2870`) calls +0xfa0 with the command; `FUN_1401c7bf0` finds
  the table entry of 0x20d / 0x20e / 0x20f (`FUN_1401cd150`) and runs it as a first call:
  `FUN_1401c8610` (motion 0xc9, Sora captures 0x101, the ride 0xfe queued by `FUN_1403c7140`,
  target renewed), `FUN_1401c8700` (motion 0xcd, capture 0xff, effect slots 3 / 4 at bone 0x13 /
  4 on Sora at 0x41, the special strike `FUN_1401caac0` each frame; a replaced motion restarts
  the enemy stop 0x11, `FUN_1403dbaa0`), `FUN_1401c8440` (point 0x140724330, motion 0xf2,
  capture 0x100, slots 0xc / 0xd, the strike).
- `FUN_1401c9920` (10:13) in a limit (+0x124 bit 16) adds Sora's Keyblade muzzle (effect 1 at
  0x4003) and a second shot for events 0 and 1; when the target is gone it is renewed
  (`FUN_1401c96c0`: Sora's lock-on target if new to the 32-entry list, else the first live enemy
  not on it, else the next live one after the current); each shot strikes the target with attack
  0x373 (`FUN_1401b8250` -> `FUN_1403cf830`, `FUN_1401b82f0`) from the view's origin moved
  (rand − 0.5) × 2 × 200 (`DAT_140585efc`) on x and y, and a landed strike starts effect 2 of
  Stitch's container.
- During a limit `FUN_1401b15f0` holds the think and acts only while `FUN_1403d7280` has a hooked
  thread (+0xd48); otherwise the friend's update runs as usual.

### The summon's appear event

- `FUN_1403de610`: the Summon Effects option is save +0x41a4 bits 9..10 (`FUN_1402e4fa0`); 1
  plays the event every time, 2 the first time each summon comes (`DAT_142a24ed8`, bit
  `DAT_1409acdd5`). The event waits SSTM +0x84, ends Sora's casting effect (`FUN_1402be8a0`),
  starts the summon at Sora's place with act `appear`, sets the state to 1, hides HUD layers
  7..10 (`FUN_14017b440`) while `DAT_142a11478` holds, then shows them again; otherwise SSTM
  +0x88, `appear_skip` at the placement and Sora's motion 0x82.

### Command menu pens

- `FUN_14015df90` builds list rows with `FUN_14015aab0` for list decks of kinds 5 and 10 and with
  `FUN_14015aba0` for kind 0xE (list kinds: `FUN_1403b1780`, mask 0x7e420). `FUN_14015aab0`'s pen
  x comes from the layout's style (`FUN_1401534b0` over `DAT_1409babb0`, the loaded
  `<world><n>command.2dd`) when the world menus are on (`FUN_1402e5380`: config bit 6 clear).

### Party

- Donald's action table (static constructor `FUN_140009d40`, entries from 0x1407250a0): Fire,
  Blizzard and Thunder entries carry flag byte 2, Cure's 0x10 (runner `FUN_1401cc1a0`), the item
  entries 0x18 / 0x28. `FUN_1401b2880` tests an entry's record range only when the flag byte has
  bit 1, so Cure's record (60, range 0–0) is never range-tested (issue #78).
- `FUN_1403f6d00` -> `FUN_1403c4ab0`: items 5..7 and 0x83 heal every slot of the current party
  (no range) by `FUN_1403c4b80`, with the user's Item Boost.

## What the rewrite does

- `enemy::world::MagicThread`, `World::magic`, `World::tick_magic` (the 0x5014 task), trap 2:20
  creating the thread; the script brain runs the list after its objects, the limit scene before
  its program thread.
- The script brain tracks its effect handles (`BrainFx`): `effect_is_active` / `effect_pos`
  answer from the room's effect update (as the limit host's), and the summon host plays every
  effect call from the PAX of the object whose script made it (`obj/<model>.a.us`), bound to the
  summon (`Anchor::Summon`) or placed where another object stands.
- `Sysobj::limit_stand` (modes 0x3e / 0x3c); 10:3 / 10:4 for Chicken Little carry him on Sora
  and set the flags; the host turns Sora to the camera's forward each limit frame.
- `enemy::friend::stitch`: `start_limit`, `end_limit`, `command` (the callback), `run_command`,
  `cycle_target`, `shot`; `Friend::call` and `Friend::targets`; `friend::STITCH_DECK`; queued and
  looping captures (`World::capture_motion`, `World::queue_capture`, `Capture::next`); the host
  draws Sora at the second matrix's point and offers Stitch's deck (mode 4).
- The limit scene runs an unhooked summon's friend update and acts during its limit.
- Summon motion attacks: the summon's clips carry its motions' attack windows (frame trigger
  range 10) and its model's ATTACK shapes; open windows land on the room's enemy once per swing
  (`summon::land_swings`).
- The appear event by the option (the room keeps the camp Config's bits, new game 0x40b),
  `state::APPEARING`, Sora held and playing the event's captures.
- `field_command_menu::layout_style` / `world_pen` for kind-5 rows.
- Cure's range note in `party_members::eligible` now cites the table evidence; Sora's party items
  heal Donald and Goofy too.
- `--summon-tour` takes Stitch's two deck rows and the reaction command in turn.

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new: the magic list's two tests, the layout pen table) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--summon-tour` on `KH2RW Launcher.exe` (private `runs/s7`) | exit 0, no faults; 4 summons, 4 appear events, 5 limit starts, 64 follow-ups, 93 summon hits (1261 ×33, 988 ×25, 1444 ×15, 883 ×13, 1210 ×4, 1225, 1573, 1574) |
| Unattended `--party-limit-tour` on `KH2RW Launcher.exe` (private `runs/l1`) | exit 0; the five party limits start, take 27 follow-ups and end; no missing traps and no faults |
| Unattended `--summon-tour` (private `runs/s1`) | exit 0, no faults; Chicken Little's 15 shot follow-ups start 15 magic threads (2:20), each creates its attack and effect and strikes the Shadow (ATKP 1444 ×15); every summon effect resolves in its PAX (54 starts, none missing, no unsupported program) |
| Unattended `--summon-tour` (private `runs/s2`) | exit 0, no faults; Chicken Little rides with Sora through his limit and makes the `surprise` rise and fall after it |
| Unattended `--summon-tour` (private `runs/s3`) | exit 0, no faults; Stitch's limit takes 525 ×4, 526 ×4, 527 ×1 through `friend_call`, plays 0xc9 / 0xcd / 0xf2, strikes with 0x373 ×13 and 0x4ba ×4; his BUTTON deck shows Ukulele and Shoot; the other three limits as before |
| Unattended `--summon-tour` (private `runs/s4`) | exit 0, no faults; summon swings land: Genie's limit 25 hits (ATKP 988), Peter Pan 19 (1135) + 1225 + 1573 |
| Unattended `--summon-tour` (private `runs/s5`, `runs/s6`) | exit 0, no faults; with the new-game config every summon takes the `appear` path once; Genie's and Peter Pan's events run (Sora plays their captures), Chicken Little's and Stitch's scripts have no `appear` act and end at once |

## Uncertainty

- [I] A magic thread whose owner object is gone is dropped (the executable keeps running it on a
  dangling owner). Threads use frame slot 15.
- [I] Effects bound to a summon take its root (its bones are not sampled for effects), lifted by
  Sora's body heights; an effect another made object starts is placed where that object stands.
- [I] Sora's turn in Chicken Little's limit reads the view matrix words as the camera's forward on
  the ground; Sora is hidden then.
- [I] Stitch's limit: Sora is drawn at the second matrix's point with the same placement as
  Stitch (view space × 0.503644 in front of the follow camera, model at its own size); the w
  column, pitch of the identity orientation and the camera-space size against the installed game
  are not compared. Sora's Keyblades are drawn from the captured weapon motion relative to his
  origin. The strike's origin point is computed but not used (the hit lands where the enemy
  stands); effect 2 starts at the enemy's position. Whether sheet mode 4 makes a list deck
  (`FUN_1403b1780` kind) was not settled: Stitch's rows are shown and taken as Chicken Little's.
  Stitch's camera-space motion windows are not landed.
- [I] Summon swings use Sora's attack stats, as the summons' other strikes; Stitch (camera
  space) is left out.
- [I] The appear event lasts while the summon's act is `appear` (the executable waits on
  `DAT_142a11478`); the HUD layers are not hidden [Open]; `DAT_1409acdd5` is read as the
  summon's `sumn` row; Sora's +0x9d8 test is not made.
- [I] The world pen applies to Chicken Little's deck (kind 5 read as his deck's kind).
- Not traced: Zantetsuken (ability 190) and Zantetsu Counter (193). No PTYA record of any set,
  no line of the `limitform` script and no command row references them; the searches for code
  loading 190 / 193 found only motion ids.
- Not done: Limit Form's limits stay the T113 hand translation (not run on the script engine);
  equipment and Customize gating of Donald's and Goofy's abilities (#77; the room has no party
  equipment or Customize frequency data); Sora's party-item heal is not exercised by a tour.
- No comparison against the installed game's frames or timings; no owner playtest.

## Verdict

PARTIAL. Chicken Little's shots fly and strike as threads of his own script, the summons' script
effects draw, Chicken Little rides Sora in his limit, Stitch's limit runs its deck, commands,
shots and Sora's ride, the summons' melee and limit swings land, the appear event plays by the
Summon Effects option, Cure's range question is settled and Sora's party items heal the party.

## Next task

A controlled comparison of the four summons and their limits against the installed game;
Zantetsuken / Zantetsu Counter from the executable; Limit Form's limits on the script engine.
