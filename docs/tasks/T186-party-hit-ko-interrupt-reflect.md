# T186 — Donald and Goofy being hit: reactions, KO and revive, interrupted casts, the Blizzard shot's reflect

Owner Claude, 2026-10-17, requested by burba in the project chat (GitHub issue
#76, which holds the old #11). Gate: development of the test room only; no
final gameplay acceptance is inferred. Prerequisites: T108 (Drive forms keep
`MemberRt.stored`), T136 (Donald's and Goofy's acts, `party-01` specs), T140
(Sora's death), T163. Build identity: `origin/main` at `554d3bf`, package 0.1.2,
Rust 1.99.0 Windows MSVC. Source: the selected Steam KH2FM build, validated at
each run; the install was only read.

Private evidence root: a private folder outside the checkout (Ghidra runner, the
decompiles under `out/`, tour runs `run*/`), and the earlier `party-01` specs
(`donald-spec.md` §8, `goofy-spec.md` §8) and `combo-01/g`, `damage-02/g`,
`guard-01` under `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/`.
Nothing from the game is in Git: the tour reads the motions, models and tables
from the installation at run time.

## What the executable does

- A hit on a member is the shared BTLOBJ pipeline reached through
  FRIEND vt+0xc0 (`FUN_1401b0290`: the personality's hit callback, which is an
  empty function for Donald and Goofy, then `FUN_1403c34d0` → `FUN_1403d60c0`).
  A dead member (`obj+0x9b8` bit 2) or one that cannot be damaged
  (`FUN_1403d65a0`) only takes the cure types (ATKP Type 5/6). Otherwise the
  record check, the HP change `FUN_1403d3ba0` (clamped by `FUN_1403c0860`), the
  KO handler and the reaction `FUN_1403d2fe0` follow.
- Reaction classes (`FUN_1403d2fe0`, friend case): 1 a knock-slide only; 2 a
  flinch without a slide; 3 a flinch with a slide on the ground (an air flinch
  and hop in the air); 4 a blow (physics mode 4). Motions: flinch 10-13 (front
  high, front low, back high, back low, `DAT_14074c390`; the partner when one
  is missing), air 14/15, blow 16 (front) and 18 (back) landing on blow + 1
  (`FUN_140422700`). The member can act again at the motion's range 2.
- HP 0 (`FUN_1403d2eb0` → vt+0xb0 `FUN_1403c33d0`): the dead bit and the revive
  timer `obj+0xdc0` = PRTY word 6 (Donald 3600, Goofy 2700). The DEAD motion 54
  starts only when the member can act and is on the ground
  (`FUN_1403c3660`, `FUN_1403bfb30`, `FUN_1403ba9a0`) with the 00effect marker
  element 14. The timer falls by SSTM+0x5c (8.0) a tick, by 1.0 while the
  battle flag `DAT_142a11404` is set. At 0, `FUN_1403c3bd0` heals
  `maxHP × SSTM+0x58 (0.5)` through `FUN_1403d2eb0`, which for a dead party
  member calls `FUN_1403c3c30`: the dead bit clears, motion 55 plays from 54,
  and the heal is scaled by SSTM+0x54 (0.5) unless the Hyper Healing flag
  (`FUN_1403d7470(obj)+0x10` bit 3) is set. A Cure or any heal revives the same
  way. (Disassembly of `FUN_1403d2eb0` confirms `FUN_1403c3c30` receives the
  heal amount in `edx`.)
- The battle flag (`FUN_1403ab4a0`) is on while an enemy (team 2) lies within
  1200 of Sora or a party member's attack object lives (`FUN_1403c5bd0`), and
  follows that state after 45 ticks to switch on and 120 to switch off.
- A cast is interrupted by the member's motion changing. Donald's acts wait on
  `sysobj_motion_id == m`: the wait for the release event (`L2202`) and the
  delays after it (`L2284`: four ticks before Fire's ring, ten before Thunder's
  strike) end when the flinch replaces the cast motion, and the act then skips
  its effects, MP and attack objects and ends. The act manager also pauses
  every act below priority 300 while the owner plays a damage motion
  (`FUN_1403ca7c0`: motions 10-39 and 0x8c-0x96 for a STDOBJ owner, bit 1 of
  `obj+0x6c8`, which the STDOBJ constructor sets): its timers stand still, its
  attack objects keep their last position, and it resumes afterwards. Blizzard's
  shot is a thread of its own (`magic_start_thread`) and is not paused.
- The shot's end and the reflect: the shot thread leaves its loop when
  `attack_is_hit_bg || attack_is_reflect || (pierce == 0 && attack_is_hit)`;
  its cleanup (`L4712`) kills the effect and frees the attack object.
  `attack_is_reflect` is the attack object's flag bit 9, set by
  `FUN_1403d2140` when it clashes with another team's attack whose Type pairs
  in `DAT_1405c5e98` (0-0, 0-2, 0-3, 1-3). Donald's shots are ATKP Type 1, so
  only a Type 3 super guard reflects them. The branch after
  `attack_is_reflect` that builds a new shot from the reflected one (`L3287`)
  tests `-1 >= 0` and never runs in the shipped script: a reflected shot only
  ends. ATTACK_MAGIC objects carry flag 0x100 (`FUN_1404371b0`), so the
  caster has no clash reaction either.
- Goofy's shield: his idle, walk and jump motions carry a weapon window with
  ATKP 139 (Type 2 guard, refact fields 0). A Type 0 claw touching its sphere
  clashes (0-2): the claw joins Goofy's record, the Shadow plays its
  ReflectMotion and `mode_revenge` (`FUN_140411060`), and Goofy takes the bounce
  branch of `FUN_1403d61d0`: effect 13, motion 41 and a knock-slide of 50
  backwards along his own facing (`FUN_1403d4650` with motion -1).
- Who the Shadow fights: `OBJ_DAMAGE` starts `mode_revenge` for a hit by the
  player character (`0x6c8` bit 7) and `mode_battle` for any other attacker;
  `mode_revenge` searches `target_search(obj, 1)`, the last team-1 attacker
  (`FUN_140411300` stores it in `obj+0xd40`) when it is alive, and `mode_battle`
  searches Sora again.

## Rewrite

- `player::party_hit` (new): the reaction classes, the motion tables, the
  per-tick reaction (flinch slide, air flinch, blow flight and landing, bounce,
  get-up), the SSTM KO rates, `suspends_acts`. `player::party_members::Follower`
  gains `react`, `bounce`, `ko`, `revive`, `revive_due` and the locked/down
  motion states; `Scene` carries the actionable ticks, SSTM, the rates and the
  battle flag.
- `player::party_spell::Cast` ends at a changed motion in the waits, pauses (and
  holds its attack objects) while the owner is in a damage motion, and ends the
  shot when `Context::reflected`.
- `player::shadow`: `hit_by` (a hit by a party member starts `mode_battle`),
  `attacker_pos` and the revenge target.
- `test_room::party_room`: the HIT spheres per motion tick, the reaction
  motions, PRTY word 6 and the SSTM rates at load; `receive` (shared by the
  Shadow and the gallery enemies) lands attack spheres on the members, runs the
  shield clash, the damage, the KO and the reaction; the KO marker; `heal_member`
  (a dead member is revived); the battle flag; the revenge target bookkeeping.
  `enemy_battle_hits` feeds the gallery enemies' attack spheres and their
  Type 3 guards.
- `--party-hit-tour <dir>`: synthetic claw spheres of the Shadow's own ATKP
  rows land on a member or its shield when each case says; the final nine
  hundred ticks are the Shadow's own fight.

## Second pass (same task)

- Voices: the members' hit voices come from the motions' frame trigger 14 (flinches, attacks, the
  get-up), the low-HP voice 9 (`FUN_1403c34d0`), the KO voice 2 (`FUN_1403d5dc0`) and the casts'
  first call, trap (1,262): 6, and 21 or 3 for Cure. They use the same vtbl rows and
  `FUN_1403d69b0` path as Sora's, with `voice/us/battle/<world>0_donald|goofy` (the exe's
  `%s%d_%s` and its name table sora, donald, goofy). Goofy's script has no voice trap.
- The air flinch lands on motion 5 (the mode-1 land motion, `obj+0x11c`, `FUN_1403cbec0`).
- Reflect in the room: `Game::enemy_guards` carries the Type 3 attack spheres of gallery
  enemies; a Blizzard shot clashing with one ends (`party_reflect`), and the tour's reflect case puts
  such a sphere in the shot's path.
- Gallery enemies: a member aims at the nearest of the Shadow and the gallery enemies within 800
  and its melee windows and spell volumes hit them through the bridge Sora's attacks use (the
  member's own stats; Sora's combo and Drive are not fed). `--enemy-combat-tour` keeps the party out
  (it is Sora's exercise) and still passes.

## Third pass (same task, PR 109)

What was added after the second pass, with the evidence for each. Every static
reading is of the Steam build named above; nothing from the game is in Git.

- **Record windows corrected.** The records' height windows are Y-down in the
  executable (`obj+0xe40` is target minus self in Y-down, `FUN_1401b7ad0`), so the Y-up window is the
  negated pair: ground -100/150, Fire -100/200, Blizzard -100/150, Thunder -500/1000,
  Tornado -100/500, Turbo -100/150 (the second pass had the pairs swapped).
- **Air attacks (item 5).** Donald records 2 and 3 (motions 181, 182, far 150/130) and Goofy
  6 and 7 (181, 182, far 100/80) need the target 100-385 above the member (raw window -385/-100).
  `FUN_1401b0420` makes the jump call `vt+0x18` with height `1 + rise` (at most 385) and 8.0
  horizontal toward the target; the member plays motion 4 while falling and motion 5 on landing,
  and a hit cancels it. Rewrite: `player::party_hit::Airborne`, `Follower::air`.
  PASS `--party-hit-tour` (`run12`, `run18`): Donald's record 3 rose to 200.9 and landed on motion 5,
  Goofy's record 6 the same, with captures of both flips.
- **Friend air-hit physics (item 7).** Traced rather than assumed: a friend's mover is the shared
  `FUN_1403b89a0` driven by the mode table `FUN_140409d20`, the same code Sora's air flinch uses,
  so reusing Sora's air-flinch data (`Arc::launch(50)`, `strength x HitBackToJump`) is the same
  call, not an approximation. The earlier "separate physics not traced" note is withdrawn.
- **Party items (item 2).** The personality table's entries 2 and 3 are `FUN_1401cc970` (HP) and
  `FUN_1401cce90` (MP), flags 0x18 and 0x28, in the same random candidate pool as the attacks and
  in the emergency passes of `FUN_1401b3a30` (flag 0x10: HP item and Cure; 0x20: MP item) that run
  first when a partner is in need (`obj+0xf78`). A member whose own HP, Sora's or the other
  member's is under 45% (`DAT_1405b41c0`; MP the same and alive) uses the first item of the
  preference list that Sora's item slots hold (`FUN_1403c4380`): HP `[1,2,5,4,7]`, or
  `[5,1,2,4,7]` with two in need; MP `[3,6,4,7]` or `[6,3,7,4]`. The target is the partner in need
  (HP), else the first of itself, Sora, the other member. The use needs an enemy target
  (`FUN_1401b2ce0` checks `obj+0xc`) and plays motion 9; trigger 6 takes the item from the
  last matching slot (`FUN_1403d7e50`, `FUN_1403c49c0`) and queues a heal 30 ticks
  (`DAT_140623bcc`) later (`FUN_1403f6790`, `FUN_1403f6960`, `FUN_1403c4ab0`): the whole party for
  items 5, 6, 7, one target otherwise, a share of the target's maxima like Sora's
  (`FUN_1403c4b80`), the item's own effect element on each target. Rewrite:
  `party_members::{ItemNeed, Urgent, item_pick, emergency}`, `party_room::PendingItem`.
  PASS `--party-hit-tour` (`run14`): Donald's Potion on himself at 40% (8 HP), on Sora at 20%
  (`target` 0, the emergency pass), Goofy's Ether at 30% MP; `party_item_use` and
  `party_item_heal` events with the slots emptied. [U] The per-item "frequency"
  byte of the character record (`+0xf5 + item index`: 0 often, 1 seldom, 2 never unless a partner is
  in need) is taken as 0: the menu that sets it is not built. [U] Item Boost for a friend user
  (`FUN_1403ba720`) is taken as 100%.
- **HUD gauges (item 1).** `FUN_14016c3a0` makes two `dk::GAUGE_FRIEND` objects from the same
  gauge sequence as Sora's: root intro group 0x47 queued to 0x48 (0x43 at low HP), ring sprites
  0x40 (shadow), 0x41 (HP), 0x42 (MP) behind five circle masks (`FUN_140181d20`), the member's
  own face sequence (`obj/<model>.a.<lang>`, `FUN_1403b57b0`) with groups 3/0/2 by state
  (`FUN_14017eba0`), the second gauge one group pitch (`+0x20` of group 0x47) lower. Per frame
  `FUN_140182730` wipes the HP half by `r x pi - pi/2` (with the half-plane rectangle for r < 0.5)
  and the MP half by `(1 - r) x pi`; the HUD events of a hit (`FUN_1403d60c0`: dead or blow class
  = group 0x44, flinch class = 0x45, other = 0x46) and of a heal (`FUN_1403d2eb0`) restart the root,
  the shadow ring and the face. **Face placement:** `FUN_14017e980` only sets the face texture's
  VRAM addresses (the sums of `DAT_14071ae10`/`ae20` per slot plus 0x3500/0x2800); the on-screen
  place is the sequence's own, so nothing was unresolved. Rewrite: `visual::gauge::FriendGauge`.
  PASS release tour captures (`run15`): both gauges beside Sora's, Donald's KO face and HP ring.
  Not built: Goofy's slide up when Donald is absent (groups 0x49/0x4a, `FUN_140182730`) and the
  per-frame chase of the shadow ring's value (the shadow keeps the HP of the event before).
- **Goofy's Bash (item 4).** The `blade` act of `p_ex030` (L3406): motion 192 (blend 8),
  `trap_obj_thread_start` (1,82) of the projectile thread (L3506), which waits for frame trigger 9,
  takes the shield from weapon slot 0 (`FUN_140431310`), detaches it (1,102) and the motion list
  (1,236), starts motion 201 on it, places it at the hand (`FUN_14042f6f0`) with ATKP 1161 (radius
  60) bound to it, and flies it with the shared homing library (turn 0.5236, speed 20, 180-tick
  limit); the hit callback L4373 stops the flight and keeps the shield on the target while hits
  keep coming within 16 ticks; then motion 193, the shield re-attached (1,126) (1,18), and idle.
  Instead of a hand port the act runs on the object-script engine (`enemy` crate, the interpreter
  the opt-in `ScriptBrain` uses): new traps `obj_thread_start`, `obj_weapon`, `obj_detach`,
  `obj_attach`, `is_attach`, `motion_sync`/`unsync`/`is_motion_sync`, `party_hand_to_bone`,
  `obj_apply_bone_matrix` (through `Env::bone_point`), `obj_use_mp`, `is_summon`; the
  motion queue is now consumed when a clip ends (`FUN_1403c6740`; it was only recorded); an
  attack's hit callback can be run by the host (`Object::run_hit_callback`). Host:
  `test_room::party_blade` (clips, floor, enemy centre, hand), `Follower` follows the engine's
  motion and position (`Scripted`), the shield is drawn from the weapon's own vertices, its ATKP 1161
  volume lands on the Shadow and on gallery enemies and a landed hit runs the callback next tick.
  PASS `--party-hit-tour` from the Bash case (`run18`, before the last clippy refactor): the act
  started at tick 1, the shield left the hand at tick 80, ATKP 1161 hit the Shadow every 16 ticks
  (12 hits), motion 193 at tick 280, idle at 351. 4 new unit tests in the engine (weapon traps, the
  queue) and 2 in `party_members`. [U] The bone number 3 of the hand lookup is not mapped: the
  member's SKLT hand bone stands in; the shield's spin uses Goofy's motion 201, not the weapon
  set's; `trap_obj_use_mp` is recorded and 10 MP is taken at the start (the executable's consumer was
  not found); the shield's second movement request (`+0xa58/+0xa64`) is applied by the host as a
  displacement of `direction x speed` a tick (the executable's consumer is not translated).
- **Spell sparks on gallery enemies (item 6).** A member's spell volume starts the element of its own
  PAX chosen by the hit-effect category of the ATKP row (`attack_set_pax`, `FUN_1402c6730`, mask
  `1 << (32 + effect - 1) | 1`) at the contact point. Coded for both the Shadow and the gallery
  bridge; **NOT RUN** against a gallery enemy (the one-off switch `KH2RW_PARTY_COMBAT` that lets
  the party into `--enemy-combat-tour` was added and not run).
- **Limits (item 3): NOT BUILT.** Traced as far as the structure: `limit/us/goofy.bar` holds the BDX
  program `limit_goofy` (triggers 23 and 22), two PAX containers (types 20 and 18), and the `comb`
  and `gage` sprite sheets; `trinity.bar` the same plus `wave` and `se40` and a 19 KB program. The
  scripts call about 50 distinct traps, including the untranslated limit group (2,47 limit_motion_start,
  2,48/49 limit_player/friend, 2,50 limit_camera_start, 2,54 limit_start_thread, 2,55 limit_light,
  2,58/59 limit_effect_start, 2,60 limit_time, 2,63 limit_reference), the camera and light traps and,
  for Trinity, the shot traps (7,28-7,36). Running them also needs the limit controller (`FUN_1403d7fa0`,
  0x128 bytes), Sora as a script-driven object, the limit gauge and combo sheets, and the limit
  motion sets. Selection and cost (`FUN_1403e7950`, `FUN_1403d8b40`, `limt` rows) are in
  `goofy-spec.md` 7.2. None of it was started in code.

## Checks

- PASS `cargo fmt --all --check`, PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  PASS `cargo test --workspace --locked`, PASS `cargo build --workspace --locked`, PASS the
  `--frames 180` smoke run. New tests: 10 in `player::party_hit` (flinch front/back/high/low, partner
  fallback, classes 1-3, blow flight and landing on blow + 1, the actionable tick, the bounce, KO rates,
  revive amounts, damage-motion suspension), 4 in `party_members` (a hit locks the member until the
  reaction lets it act, KO then DEAD then timer then get-up, timer 1 a tick in battle, bounce), 6 in
  `party_spell` (a hit before the release or in the delays ends the act with nothing spent or started, a hit
  after the ring's start pauses it with its attack in place, the shot flies on while the act is paused,
  a reflected shot ends with no second attack object), 2 in `shadow` (party hit starts `mode_battle`;
  revenge targets the last attacker), 1 in `party_room` (battle flag 45/120).
- PASS release `--party-hit-tour` against the installed build (`run6`, 45 captures, log
  `party_hit_*` events): a claw on Donald 12 ticks into Fire interrupts it (`interrupted_cast`, no
  `party_release`, flinch 10); a hit 30 ticks into Thunder (release at 26, strike at 36) also ends it
  with nothing spent; a Blizzard-shot blow on Goofy plays motion 18 (back blow) and a claw plays 12;
  Donald at 1 HP struck: `party_ko` with timer 3600, `party_revive` by the timer 450 ticks later (8 a
  tick outside battle) with 4 HP (a quarter of 18); Goofy KO'd (timer 2700) and raised by Donald's Cure
  (`party_heal` `was_down`, 10 halved to 5 HP); a claw on Goofy's shield from the front gave
  `party_guard` with motion 41 (the same claws from behind, outside the 45 degree cone, hit his body); a
  hit on Donald 36 ticks into a Fire cast (ring up) interrupted the act; the Shadow's own fight at the
  end hit Donald by its claws (`party_hit_taken` at tick 2077 in run 4 and again in run 6).
- Compared with the executable (static): `FUN_1401b0290`, `FUN_1403d60c0`, `FUN_1403d3ba0`,
  `FUN_1403d2eb0` (with disassembly), `FUN_1403d2fe0`, `FUN_1403d43b0`, `FUN_1403d3840`,
  `FUN_1403d4650`, `FUN_1403d61d0`, `FUN_1403c3660`, `FUN_1403c3bd0`, `FUN_1403c3c30`,
  `FUN_1403ca550/7c0/250`, `FUN_1403d2140`, `FUN_1403d1f40`, `FUN_1403ab4a0`, `FUN_1404371b0`,
  the Friend vtable slots, SSTM +0x54/+0x58/+0x5c (0.5, 0.5, 8.0), PRTY word 6, and Donald's BDX acts
  (`fire` L2003, `blizzard` L2571 and its shot L3166-L4712, `thunder` L5052, `cure` L5453).
- PASS release `--party-hit-tour` (`run7`): 31 `party_voice` / `party_hit_taken` sound events with
  Donald's and Goofy's banks (for example 9 then 2 at the KO tick), and the reflect case: Donald's
  Blizzard cast at tick 1783 ended with `party_reflect` at 1808 when the shot met the guard sphere.
- PASS release `--enemy-combat-tour` (`run10`, verdict pass with positive damage both ways), and a
  run with the party in (`run8`): Donald's Fire hit a gallery actor (`gallery_hit` with `member` 0) and
  gallery actors hit Donald and Goofy (`party_hit_taken` from ATKP 405 and 7, a blow with motion 18).
- NOT RUN: owner playtest; comparison with the installed game's frames or timings.

## Not reproduced / uncertain

- Third pass, checks that were NOT RUN: the full `--party-hit-tour` after the last edits (the
  clippy refactor of `party_blade::Start`, `is_multiple_of`, the tour's skip switch
  `KH2RW_PARTY_TOUR_FROM`); `--enemy-combat-tour` after the third pass; a gallery enemy hit by a
  member's spell spark or by the Bash; the item cases against gallery enemies; any owner playtest.
  PASS this pass: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D
  warnings`, `cargo test --workspace --locked`, `cargo build --workspace --locked` and the
  `--frames 180` smoke run on the final tree.
- A voice's priority (row byte 2) is not used, as for Sora's: two voices on one tick both play.
- The `patn` override (`FUN_14040a8e0`, `FUN_1403ee9b0`) is taken as absent for
  the party; reaction class 5 (Magnet's capture, `FUN_14040fbf0`) and 14 are not
  built for members.
- Air hits on a member use Sora's air-flinch data because the mover is the same
  (`FUN_1403b89a0`, `FUN_140409d20`; third pass).
- Mode 4 (blow) is assumed for the friend's blow (`FUN_140409d30` can choose
  mode 8 or 0x21 for other flag states).
- `FUN_140410100`'s counters (`DAT_142aea190`) belong to a mini-game manager and
  are not modelled.
- Hyper Healing, Auto Change, Second Chance and Once More do not exist for the
  party in the room (no equipment).
- Goofy's A201 (motion 11) starts at frame -1; it is sampled from 0.
- ATKP 139 (Goofy's shield) has ReflectRange 45, not 0 as `goofy-spec.md` §5.4 says: the
  shield only clashes with attackers within 45 degrees of his facing. The claws' own cone
  (0 for the Shadow's) is taken as met for other attackers.
- The reflect needs an enemy Type 3 attack sphere in reach; the three supported gallery enemies
  were not found to make one in the runs, so the reflect in the room was checked with a synthetic
  guard sphere in the path of the real shot.
- No comparison against the installed game's frames or timings.

## Verdict

PARTIAL.
