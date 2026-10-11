# T107 — Donald and Goofy in the test room: models, motions, following and ground attacks

Owner Claude, 2026-10-11. Gates P18/P21 (test-room scope). Starts from `main` at d0e9ee2. User request: Donald and Goofy as party members, 1:1.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `research/playable-room-20261008/party-01/donald-spec.md` and `goofy-spec.md` (Ghidra 12.1.4 on the read-only executable). Autoplay evidence is in `combo-01/autoplay-p1`.

## Findings (executable and installed data)

1. Donald (`P_EX020`, objentry 0x5c, char 2) and Goofy (`P_EX030`, objentry 0x5d, char 3) are `kn::Friend` objects; their behaviour is C++ (`kn::Donald`, `kn::Goofy` personalities), not their BDX scripts (FUN_1401b53b0 think, FUN_1401b1d40 chooser).
2. Following (FUN_1401b41a0): three gaits (stand, walk, run) chosen by distance to a goal behind the leader, speeds 2.0 and 8.0 per tick.
3. Attack choice (FUN_1401b1d40, FUN_1401b2880): no combo chain; each decision picks one eligible record at random by the target's distance, height and front half-plane, never the same one three times running. Hits come from the weapon motion's range-10 windows on the staff or shield's bone-2 spheres (ATKP 151–155 for Donald, 156–159 for Goofy).

## Implementation

- `player_sim::party::Follower`: the gait rules and movement through the shared world collision.
- `test_room/party_room.rs`: both models, their staff and shield, and the idle, walk, run and two ground attack motions are found by name in the installed archives (`obj/P_EX020.mdlx` / `.mset`, `W_EX020`, `P_EX030`), posed per tick; members follow Sora, close in on the enemy, attack, and their weapon windows land hits through the shared hit record and damage rules.

## Checks

PASS fmt; PASS clippy (warnings denied) for player_sim and windows_shell; PASS their tests; PASS release build with `--features skate3`; PASS autoplay `autoplay-p3` (exit 0; `p2` failed once on blank late captures and passed on rerun): both members load, carry their weapons and land hits (Donald ATKP 151 and 155, Goofy 156). NOT RUN: interactive playtest; full workspace test run.

## Uncertainty

- The formation distance and slot spread are test-room values (`DAT_14071d760` is filled at run time); thresholds are inferred from the personality header.
- Air attacks, spells, items, KO and revive, and Limits are not built yet; being hit and the hit reactions of the members are not either.
- Strength and Magic come from the LVUP row alone (no plrp or equipment terms); the choice RNG seed is unknown.

## Verdict / next

PROGRESS: both members appear and follow. Next: air attacks, spells and items, being hit, KO and revive, then Limits.
