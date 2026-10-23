# T208 — The reaction command: one search, its slot, marker and press

Owner Claude, 2026-10-23, requested by burba in the project chat ("start making all reaction
command systems 1:1 with ghidra"). Gate: combat and HUD (P19/P20 work; nothing here is inferred
as a gate pass). Prerequisites: T062 (command menu), T070 (field talk), T113 (Limit Form), T202 to
T204 (second deck, party limits, summons), T206 (boss framework: the REACTION shapes, their
enable test and Sephiroth's Block). Build identity: branch `claude/project-thread-kyocfc` cut from
`main` at `ec58992`, then merged with `main` at `6de4a1f` (the summons' T207); package 0.1.2, Rust 1.99.0 Windows MSVC. Source: the selected Steam build
15194255, validated at every run; the install was only read. (T207 is the summons' record.)

Private evidence (never in Git): `%LOCALAPPDATA%/KH2FinalRewrite/research/reaction-1to1-20261023/`
(plain `analyzeHeadless` decompiles `out/core1`..`core3`, `hud1`, `hud2`, `dd8`, `hold2`,
`misc1`; operand scan `out/op_dd8.txt`; references `out/refs1.txt`, `refs2.txt`; table peeks
`out/peek1.txt`, `peek2.txt`; project copies `ghidra-kh2-sep2`, `sep3`, `a3`, read only) and
the earlier `research/sephiroth-01/rc-01/reaction-command-spec.md`. No Ghidra MCP was used.

## What the executable does

The command manager (embedded in Sora's object, ticked by FUN_1403b2340 before his update) keeps
one reaction slot (mgr+0xb40). Each tick FUN_140408520 builds an empty candidate record
(FUN_1404020e0: mode 5) and FUN_1404081c0 offers it, in this order:

1. **The command taken last** (Sora +0xdd8, target +0xbf8) at priority 0xc9, mode 8. Sora's
   update sets +0xdd8 when a request made by Triangle is consumed (FUN_1403a89a0, unless the
   record's flag bit 9 is set); any other candidate that wins clears it (FUN_140408bd0), as do the
   state handlers that bring Sora back to idle (FUN_1403a81c0 callers).
2. **The manager's record 1** (FUN_1403b2660(mgr, 1)), filled every tick by scripts
   (`command_override`), motion range 20 and the limits (FUN_1403b2850), at 0xcb.
3. **The partner's command** (+0x18c bit 20, +0x638, +0x2b0) at 0xcc, mode 7.
4. Unless a party limit runs: **every object's REACTION shapes** (FUN_140407de0; kind 5 its
   argument, 0x83 the spawn record's, kind 14 a clashed attack's +0xcf0) and **every attack that
   carries a command** (FUN_140407b40; `attack_set_rc`, FUN_140437b70: +0x11c command, +0x120 /
   +0x124 sphere, +0x128 offset, +0x118 owner), at the record's priority, 0xca for a command
   whose state (range 26, +0x638) Sora is in.
5. A dimmed winner (score bit 30) shows in mode 5.
6. **With nothing found**, on the top deck or the second deck only (FUN_1403b1800): Auto Summon
   (status bit 56, item 0x185) offers a charm's summon when a member is down and the second deck
   is up (table 0x1405cca50: 0x19, 0x9f, 0xa0, 0x17f); the auto forms (table 0x1405cca40: Auto
   Final, Master, Limit, Wisdom, Valor) when HP is at a quarter or less (FUN_1403c1d30); otherwise
   Auto Limit (FUN_1403e7950: a limit whose member carries Auto Limit, status bit 129, within 500
   of Sora). Each picks the one already shown, else one at random (`DAT_140783ca0` × 0x10dcd,
   scaled as FUN_1404077f0 does), at priority 0 with Sora as target.

A candidate replaces the best on a higher priority, or a lower score at the same priority
(FUN_140408080), unless the model may not use it (FUN_1403b6a10), an event slot is busy for
commands 0x1e / 0x25c (FUN_1403ad500), or its object is inactive (FUN_1403ba4c0; not for 0x1c8).
The score (FUN_1404088f0) is the horizontal distance, × 0.5 for the locked-on target of the
leader, × 0.8 for the target already shown, with bit 30 for a failed facing gate or a no-control
command; FUN_140408bd0 adds bit 30 past the record's distance range. The mode (FUN_140401d60) is
the dispatcher case's (0 / 0x67 / 0x68 → 5; Drive, summons and limits by their own rules), then 5
for a no-control command without control, a battle-only command (+0xe bit 0) outside battle, any
command without flag bit 5 once the battle is won, the form's bit of the mask of the record the +2
chain ends on (FUN_1403b6cb0), or a locked command group (mgr+0xbe4, FUN_1403f29c0).

The slot (FUN_140408520) keeps the winner; a new command or target releases the marker; a mark
(+0x24) other than 0 (mode 8 has none) creates a dk::LockOn on `fiel` group 0x1405cca14[mark]
({0, 0x32, 0x34, … 0x42}) over the target (FUN_14017c570); +0xde0 == 6 (Antiform), the model's
flag bit 15 or +0x194 bit 7 dim the slot; a held command whose mode changes from 8 with record
flag bit 3 restarts the art (candidate flag bit 1).

Triangle (FUN_1403b2340): a menu lock (mgr+0x24 bit 1) shows the slot in mode 5; with a command
shown, mode below 4 and the record's decider satisfied (FUN_1403b6950 → FUN_1403ba070 for Sora,
the target or both), the request is made (FUN_1403d7f30), mgr+0xd takes the record's +0x1c, the
HUD's burst is requested (FUN_14015d660) and the request's flag 0x10 set; a mode 5 or 6 command
plays se 5; no command lets the press go to the menu.

The HUD (FUN_14015d670): the slot at (0x24, 0x179 − 22n); group 2 shown, 5 for mode 7, 3 for a
record with flag bit 3; group 0 fades it out once when the command goes (group 1 with the taken
command's text after a burst) and hides it at the end; the text is `[0x11, pen, ff 1c 00 09,
glyph, 11 02 00 1c 00] + message` with the pen from the layout file (FUN_140153850) and the
Triangle glyph (FUN_1402ebe70(0x1000)); the text colour 0x140717da0[mode] and the art's RGB
0x1405b1c50[mode] ({1,1,1,1,0.6,0.6,0.6,1,1,0.95}). A burst starts a ReactionDecideSprite on
group 4 at the slot.

## Inventory of the reaction command systems

| System | Original | Before | After |
|---|---|---|---|
| Search driver and order of sources | FUN_1404081c0, FUN_140408520 | shapes only | all six sources in the exe's order |
| Candidate test and store | FUN_140408080, FUN_140408bd0 | priority / score / form gate | + event-slot refusal, inactive target, distance dim, hold clear |
| Score | FUN_1404088f0 | translated (T206) | the 0.5 factor only for the leader |
| Mode | FUN_140401d60, FUN_1403b6cf0, FUN_1403b6cb0, FUN_1403f29c0 | no group lock, mask of the record itself | shared tail, mask through the +2 chain, group lock; Drive / summon / limit cases from the room's existing rules |
| Model gate | FUN_1403b6a10 | translated (T206) | unchanged |
| Collision shapes, spawn and clash commands | FUN_140407de0 | kind 5 only | + `shape_command` (0x83, kind 14); the room's actors carry neither |
| Attack-borne commands | FUN_140407b40, FUN_140437b70 | not done | offered from script attacks with `attack_set_rc` |
| Command held after Triangle | FUN_1403a89a0, FUN_1403a81c0 | not done | priority 0xc9, mode 8, cleared as above |
| Manager record 1 (limits, summons, party limits) | FUN_1403b2660, FUN_1403b2850 | each module drew and took its own offer | one candidate at 0xcb through the search |
| Partner command | +0x18c bit 20 | not done | in the search; the room has no partner state |
| Auto Summon / auto forms / Auto Limit | FUN_140407a90, FUN_1404079a0, FUN_1403e7950, FUN_1404077f0 | not done | offered and carried out |
| Slot, marker | FUN_140408520, FUN_14017c570, FUN_14017bfe0 | no marker | marker sprite with release outro |
| Triangle | FUN_1403b2340, FUN_1403b22d0, FUN_1403b6950, FUN_1403ba070 | Talk, drive, summon, party limit and enemies each read Triangle | one press; decider and window test |
| HUD slot and burst | FUN_14015d670, FUN_14015d660, FUN_140153850 | group 2 at x 28, dim 0.6 | groups 0/1/2/3/5, x 36, text colour and RGB by mode, prompt pen and glyph, burst group 4 |
| Talk (field) | cmd 31 through FUN_140407de0 | hard-coded sphere test | a shape in the search |
| Sephiroth's Block / Split Flash | collision entry 18, range 4, trigger 11 | T206 | unchanged, now through the one search |
| Script enables (1:49..54), callback (1:325) | FUN_14040a590…, FUN_1403b5fa0 | T206 | unchanged |
| Save-point heal while "Save" shows | FUN_140418f30 (command 0x37) | not done | not done (no save point in the room) |

## Steps

1. Player crate (`reaction_command`): the record's remaining fields; the candidate record; the
   search with the hold, record 1, the partner, shapes, attacks and the auto tail; the slot with
   its marker; the press, the decider and FUN_1403ba070; the hold rule; motion range 26 as
   Sora's state (`Player::reaction_state`).
2. Visual crate: the reaction slot draw and burst (`field_command_menu`), the prompt pen
   (`reaction_pen`), the marker sprite (`lock_on::Marker`).
3. Room: one system before Sora's update builds the inputs (actors' shapes and attacks, the
   Talk NPC, record 1 from a party limit, a summon or Limit Form, the auto abilities) and
   dispatches a press to its source; Talk, the limits and the summons read that press instead of
   Triangle; the marker is drawn with the reticle.

## Checks

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo build --workspace --locked`, the release helper
(`build-release-skate3.bat` → `tools/Build-Launcher.ps1 -Skate3`), `python tools/progress.py`
and its tests, and unattended tours against the install (results below).

## Uncertainty

- Nothing here was compared with a capture of the installed game; the behaviour comes from the
  executable and the install's data.
- **Correction to T206.** T206 took FUN_1401b15f0 to be where Sora's request waits for a command
  window. That routine reads the +0xe18 flags and lives beside the kn::Friend class
  (FUN_1401b06e0): it is the party members' update. Sora's own update (FUN_1403a89a0, in the
  range of the leader's constructor FUN_1403a7a40) holds the command and runs FUN_1403d6fc0 on
  the request in the tick it was made. A press is now carried out at once (the press itself
  already tested his windows); before, a press outside a window was dropped.
- The hold's end. FUN_1403a89a0 clears +0xdd8 for any request before it holds a Triangle one;
  FUN_1403a7330 (slot 0 of three player-class vtables, 0x1405c47d0 / 4980 / 4b30) clears it at
  the object's reset; FUN_1403a81c0 is called from the classes' KO handlers (FUN_140404d60 at
  Sora's vt+0xb0, FUN_1404051e0, FUN_140403300 / FUN_140403b00) and eight per-class thunks. The
  room ends it on Sora's next command (an action he starts that is not movement, a landing or a
  hit) and on his KO. [I] It also ends it when a talk ends (the talk branch was not checked).
- While a summon's limit scene captures Sora, the window test counts as open and the no-control
  test as passed [U]. His captured clips do carry the states the follow-ups chain to (Genie's 543
  → range 26 = 535 at frames 96..136, Stitch's 527 → 600 at 30..102), but in the room the
  summons' scripts capture him anew before those windows open (Genie: a new capture 15 ticks
  into the 136-tick one). With the executable's tests no follow-up was ever taken and the
  limits stalled (the last summon's limit never started), so the summon-limit flow has to be
  checked first. The scene's proxy of Sora holds no motion flags (+0x18c 0x4000, +0x638 0).
- Found on the way: Sora's captured motions in summon limits were shown on frame 0 throughout
  (the frame came from the summon's clip table, which holds no length for those slots); they now
  play from the capture's own length. The scripts' flow does not read that frame (the same runs
  before and after).
- FUN_1403b6a10's +0xde0 == 0xc case ("no reaction at all") is a state the room's forms never
  take [U]; Antiform (+0xde0 == 6) dims the slot as FUN_140408520 does.
- The partner command (+0x18c bit 20) and the clash (kind 14) and spawn (0x83) commands are in
  the search, but the room's objects never set them. An attack whose owner was moved to another
  object with `attack_set_rc_owner` is not resolved to that actor.
- FUN_140401d60's cases with state are taken from the room's existing rules: the Drive command's
  (case 4, `drive_room`), the summons' and the limits' (`second_deck`); the other cases count as
  0 (but 0 / 0x67 / 0x68, which are 5). FUN_1403c3950 (a member's readiness for case 0x2c3) was
  not traced.
- The room's party members carry no ability record, so their Auto Limit counts as equipped, as
  the Limit list already assumes [test-room choice]; Auto Summon and the auto forms use Sora's
  status bits as the room keeps them.
- The menu lock (mgr+0x24 bits 0 and 1), the command-group lock (mgr+0xbe4), the event-slot
  refusal (FUN_1403ad500) and the model flag bit 15 are wired but always clear in the room.
- The save point's heal while "Save" (command 0x37) shows (FUN_140418f30) is not ported: the
  room has no save point.
- The ReactionDecideSprite (vtable 0x1405b1bc8) adds only a pool return (FUN_14015b460) and a
  destructor (FUN_14015a9a0) to the generic sprite; its update is FUN_14016b3c0, which deletes it
  at its group's end, as drawn here. The custom-layout second burst (group 99 at the root) is
  not drawn.

## Results

- fmt, clippy with `-D warnings`, workspace build: PASS. Workspace tests: 970 passed, 0 failed,
  1 ignored after the merge (967 before it) (the test `main` already ignores). Smoke (`--frames 180`): exit 0. Release helper
  (`Build-Launcher.ps1 -Skate3`): PASS. `tools/progress.py` and its tests: PASS.
- New unit tests: the search's order and priorities, the score, the hold in mode 8 and its
  takeover by a state-locked shape, the form mask through the +2 chain and the group lock, the
  auto forms (owned, the one shown kept, the top/second-deck rule), the random index, the slot's
  marker (restart on a new target, none in mode 8), the press (taken / ignored / refused / menu
  lock), FUN_1403ba070, the shape commands, the reaction prompt's pen.
- Unattended runs against the install (private `kh2-reaction/runs`, release build of this
  branch; "main" = the main folder's release build of `ec58992`):
  - Sephiroth (`--sephiroth-tour`): Block (664) is offered during Split Flash at tick 267–269,
    the slot shows "Block" with the Triangle prompt and the `fiel` group 0x3a marker (a green
    glow with rays) over him; Triangle at 280 runs his trigger 11 the same tick (capture 252, then
    253 `rc_success`); the slot then holds "Block" in mode 8 until about tick 380. On main the
    same press was dropped for 12 ticks first. A full fight with `KH2RW_SEPH_TOUR_LOW=2600`
    reached his death (3024) and the mission clear (3117) in run s10; run s11, same settings,
    did not land the finishing blow by 5400 (the tour's kill is timing-sensitive, as in T206).
  - Party limits (`--party-limit-tour`): 5 limits, 27 follow-ups, the same as main; 25 of them
    through the slot's Triangle.
  - Summons (`--summon-tour`): follow-ups by command 543 ×20, 530 ×15, 479 ×12, 470 ×8 and 527
    ×20, against main's 543 ×20, 530 ×15, 479 ×12, 470 ×8 and 525/526/527 ×9. Stitch's script
    offered 527 throughout here where it alternated on main: the summon scripts' state had
    already diverged before its first offer. After the merge with `6de4a1f` (whose tour takes
    Stitch's rows and Triangle in turn) the run gives 64 follow-ups, the same commands and
    counts as main's run above; Block in the Sephiroth tour is unchanged.
  - After the hold and capture-frame changes: the summon tour again gives main's 64 follow-ups;
    Block's held slot lasts from the press (280) to Sora's next attack (430).
  - Talk (`--autoplay` on the talk room): "Talk" with the 0x32 marker (the green triangle) over
    the NPC from tick 0; Triangle at 60 starts the talk.
  - Limit Form (`--autoplay`, arena): Infinity at 7700 and one follow-up (Shoot) at 7819 through
    the slot; main gives the same single follow-up (and neither starts Strike Raid at 8060).

## Verdict and next task

PARTIAL. The command manager's reaction command (the search over every source in the
executable's order, the held command, record 1, the partner, shapes, attack-borne commands and
the auto abilities; the slot with its marker; the press; the HUD slot and burst) is translated
and runs against the install for Sephiroth's Block, Talk, party limits, summons and Limit Form.
Nothing is compared with the original game, so no behaviour is claimed 1:1 by observation.

Next: (1) the summons' limit flow (when their scripts capture Sora), so the window and
no-control tests can run there; (2) the talk branch's effect on the held command; (3) the
partner command and clash commands once the room has objects that set them; (4) the save
point's heal (FUN_140418f30) with save points; (5) side-by-side captures against the installed
game.
