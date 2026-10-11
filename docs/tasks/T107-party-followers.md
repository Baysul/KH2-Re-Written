# T107 — Donald and Goofy in the test room: models, motions and following

Owner Claude, 2026-10-11. Gates P18/P21 (test-room scope). Starts from `main` at d0e9ee2. User request: Donald and Goofy as party members, 1:1.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `research/playable-room-20261008/party-01/donald-spec.md` and `goofy-spec.md` (Ghidra 12.1.4 on the read-only executable). Autoplay evidence is in `combo-01/autoplay-p1`.

## Findings (executable and installed data)

1. Donald (`P_EX020`, objentry 0x5c, char 2) and Goofy (`P_EX030`, objentry 0x5d, char 3) are `kn::Friend` objects; their behaviour is C++ (`kn::Donald`, `kn::Goofy` personalities), not their BDX scripts (FUN_1401b53b0 think, FUN_1401b1d40 chooser).
2. Following (FUN_1401b41a0): three gaits (stand, walk, run) chosen by distance to a goal behind the leader, speeds 2.0 and 8.0 per tick.

## Implementation

- `player_sim::party::Follower`: the gait rules and movement through the shared world collision.
- `test_room/party_room.rs`: both models and the idle, walk and run motions are found by name in the installed archives (`obj/P_EX020.mdlx` / `.mset`, `P_EX030`), posed per tick, and follow Sora in rooms with a fight.

## Checks

PASS fmt; PASS clippy (warnings denied) for player_sim and windows_shell; PASS their tests; PASS release build with `--features skate3`; PASS autoplay `autoplay-p1` (exit 0): both members load and stand with Sora in the captures. NOT RUN: interactive playtest; full workspace test run.

## Uncertainty

- The formation distance and slot spread are test-room values (`DAT_14071d760` is filled at run time); thresholds are inferred from the personality header.
- Staves and shields are not attached; attacks, spells, items, KO and revive, and Limits are not built yet.

## Verdict / next

PROGRESS: both members appear and follow. Next: the action table and attacks (spec §4–§6), then spells and items, KO and revive, then Limits.
