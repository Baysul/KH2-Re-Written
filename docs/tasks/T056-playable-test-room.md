# T056 — playable Sora test room

Owner Claude, 2026-10-08. Gates P15/P16/P18/P19/P21 (test-room scope only). Starts from `main` at `510cbd6` (T055 merged). Read AGENTS.md, fidelity contract, milestones, unknowns, T053–T055 and the user-supplied rewrite guides before work. Goal: a native Windows Rust/Bevy room where the user controls Sora walking/running, jumping/landing and attacking with his own animations from the selected Steam install. KH2FM only; no Sora save exists (user, 2026-10-08).

## Prerequisites / source

- Pinned Rust 1.99 workspace; Bevy 0.19.1.
- Selected Steam root `<steam>/steamapps/common/KINGDOM HEARTS -HD 1.5+2.5 ReMIX-`; fresh full scan of the 13 candidate files, qualification `quarantined_unqualified` (unchanged). Every `test_room` launch rehashes the complete selected source once before decoding (`ProbeSource::bind_session`).
- OpenKH pin `7a3b945c538d32c6a285128c98aefba093f52ceb`: `MotionSet.cs` (slot chooser, MotionName IDs), `Objentry.cs`, docs `00battle.md` (PTYA), `preferences.md` (PRTY/SSTM), `anb.md` (range trigger IDs), `coct.md` (surface flag bits). These are public labels/leads, not installed-runtime evidence.
- The handoff's earlier private research root was not reachable on this machine; a fresh private root was created instead (below). No previous evidence was modified.

## Steps and findings

1. Located entries by MD5 name hash in the six KH2 HEDs: `obj/P_EX100.mdlx` (sixth, 578), `obj/P_EX100.mset` (sixth, 579), `obj/W_EX010.mdlx` (first, 851), `00battle.bin` (first, 0), `03system.bin` (first, 6), `00objentry.bin` (first, 3), and 278 `map/*.map` rooms. Rooms are named `map/<world><nn>.map`.
2. MSET slot survey (private): IDs 0–5 (IDLE/WALK/RUN/JUMP/FALL/LAND) are populated; battle-with-weapon mode resolves slots 0/4/8/14/18/22 through the public chooser. Frame windows: key times run at twice the declared 30 fps FrameEnd, so one 60 Hz tick advances one key-time unit (consistent with the public "60 GFR" note and the T053 preview).
3. ANB range triggers used by the controller: 11 "Allow combo" (attack chain window), 2 "Blend to idle" (LAND cancel at tick 12), 3 "No gravity" (none present in the selected aerial clips). Trigger frames are in 60 Hz units (frame triggers extend past FrameEnd).
4. Sora's PRTY record is offset-table slot 1 of `03system/pref/prty`: walk 2, run 8, jump height 185, turn 0.2618 rad (units/tick, units, rad/tick by public labels). SSTM fall max 16. PTYA set pointer 1 is Sora's (37 entries). Chosen combo entries by ID: ground 5 (A300, motion 151) → 4 (A301, 152) → 3 (A304 finisher, 155); air 9 (A330, 181) → 10 (A331, 182) → 8 (A332, 183). Entries with ability 0 and no distance requirement were preferred; the sequence itself is a test-room choice.
5. `00objentry` is not a BAR; entries are 0x60 bytes. Player entries carry WeaponJoint 1; all `W_EX010*` weapon entries carry 25. Joint 25 of Sora's skeleton sits at the idle hand height (68 units) and holds the Kingdom Key correctly once the weapon is expressed in its own root-joint space.
6. Collision: the selected map's COCT includes a large invisible plane at rooftop height. Filtering by surface flags (bit 0 partyStand for floors, bit 4 partyCollide for walls) removes it and spawns Sora on the plaza floor (level with the largest partyStand area).
7. Handedness: rendering raw source coordinates places the weapon joint on Sora's left. The test room renders the whole scene under one X-mirror root (simulation stays in source coordinates); captures then show the Keyblade in the right hand and the face on the expected side. This is an inference from the character, not an observed installed render matrix.

## Implementation

- `crates/player_sim` (new): fixed-tick controller. Idle/walk/run by stick magnitude, rate-limited turning, jump to PRTY height with gravity and fall cap, landing with blend-to-idle cancel, grounded combo chaining inside "Allow combo" windows, one aerial combo per jump with PTYA "Jump" as lift height, floor following with step height, ledge fall-off, wall push-out. Ten synthetic tests.
- `package_reader`: `ptya` reader, `objentry` reader, `motion::motion_set_index` (public chooser) and `read_motion_ids`, `InterpolatedMotion::frame_window`, `ProbeSource::bind_session`. `package_reader` builds with opt-level 3 in dev so startup precomputation of every motion frame stays practical.
- `windows_shell`: shared `room` module (room loading now also returns collision triangles with surface flags); new `test_room` binary (third-person camera, keyboard/mouse/optional gamepad, 6-tick pose crossfade, Keyblade on the weapon joint, F1 collision overlay, `--autoplay` scripted verification with captures). `room_scene` uses the shared module unchanged in behaviour.

Test-room choices (logged in every admission record): gravity 0.8 units/tick², step 24, walk threshold 0.6, body radius 30 / height 140, floor normal ≥ 0.5, blend 6 ticks, camera rig, MIRROR. None is an observed KH2 rule.

## Evidence (private, outside Git)

Root `%LOCALAPPDATA%/KH2FinalRewrite/research/playable-room-20261008/`:
`source-manifest-01.json`; `mset-dump-selection-{01,02}.json` (01 failed: identity read through a sandboxed Python, retained), `mset-dump-report-02.json`, `mset-dump-01/`; `battle-*`, `system-*`, `objentry-selection-01.json` (archive_probe rejected the non-BAR table, retained); `actor-selection-01.json`, `weapon-selection-01.json`; `map-survey-01/` (eight room_scene captures); `test-room-tt08-0{1,2,3}.json`; autoplay runs `autoplay-02` … `autoplay-11` with `.jsonl` logs and 12–13 captures each (`autoplay-01.jsonl` is an empty log from a rejected first launch, retained); `inspect-07.jsonl`.

Final-build verification `autoplay-11`: full source rehash passes; action log shows Run → Walk → Run → Idle → Jump → Fall → Land → Idle → Attack(0) → Attack(1) → Attack(2) → Idle → Jump → AirAttack(0..2) → Land → Idle; jump apex 174.9 units above the floor; captures inspected for walk, run, jump, fall, three ground swings, air attacks and a front view with the Keyblade in the right hand.

## Checks

PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked` (141 tests); PASS `cargo build --workspace --locked`; PASS `cargo run -p windows_shell --locked -- --frames 180`. PASS scripted test-room autoplay with captures. NOT RUN: interactive keyboard/gamepad playtest by the user; side-by-side comparison with the installed game.

## Uncertainty

Movement speeds/jump height come from public-labelled records; gravity, step, body size, combo sequence, blend and handedness are explicit choices. IK, inverse curves, scale, constraints, expressions, root motion, weapon constraint motion, hit detection, enemies, sounds/effects and measured installed timing remain unimplemented. Map rendering keeps T052's unlit inspection materials (sky/background planes look flat). The room is one Twilight Town map selection, not a decoded spawn.

## Verdict / next

PROGRESS: the user-requested playable test room runs from the selected install with Sora's own walk/run, jump/fall/land and attack animations under player control (scripted verification PASS; interactive playtest NOT RUN). No fidelity gate passes. Next: user playtest feedback, root motion for attacks, hit detection against a target dummy, and a reference comparison of speeds/jump timing once a Sora save exists.
