# T090 — Attack selection, combo counters, abilities and auto-target from the exe

Owner Claude, 2026-10-10. Gates P18/P21 (test-room scope). Starts from the movement branch tip (`083da4a`). User request: rewrite the entire ability and combo system 1:1 (animations, damage, magic, items, Drive), traced with Ghidra. This record is stage 1, the ground and air combo chain.

## Prerequisites / source

Same selected source and private manifest as T056–T089 (quarantined; full rehash at every launch). Ghidra 12.1.4 on the read-only executable; outputs and specs are private (`research/playable-room-20261008/combo-01/`). Background tracing for the later stages (magic, Drive, items and abilities, damage and reactions) is in `magic-01`, `drive-01`, `items-02` and `damage-02`; none of it is implemented yet.

## Findings (installed executable and data)

1. **Attack command.** Attack (request type 1) runs `FUN_1403d6fc0` → combo gate `FUN_1403d6ca0` → vt+0xf8 `FUN_1403a76b0` / `FUN_1403a8690` → context `FUN_1404028d0` → chooser `FUN_140402d20` → `FUN_1403d78d0`. Requests last one frame (`FUN_1403b2340` clears them), so there is no input buffer.
2. **Chooser.** Every record of Sora's PTYA set (37 records) is gated by `FUN_140402750` (ability bit at +0x40, air flag +4 bit 0 equals airborne, combo-limit test) and, by kind (+1), measured against shapes by `FUN_1404029f0` / `FUN_1404027f0` (distance, Y-down height and facing-cosine bands). The first marked record in file order wins (`FUN_140402c50`); if it would repeat the current motion, its alternate (+2, by id) is kept as a fallback. If nothing is marked, record 5 (ground) or 9 (air) plays.
3. **Combo counters** (obj+0xd74 step, +0xd75 finisher, cap 100). A press inside range 11 after a hit (obj+0x120 bit 25) adds a step; inside range 50 it also needs the Finishing Plus count. Otherwise it needs an active range 0–2 and starts at 1/1. Limits are stats +0x200/+0x201 (ground/air, default 3), +0x202 (finishers, default 1). Berserk Charge (bit 107) with MP charging removes the limit.
4. **Abilities.** The battler's ability record (stats +0x1d0) is reset by `FUN_1404016b0`, filled by `FUN_140401730` per equipped item (Combo Plus, Air Combo Plus, Combo/Air Combo/Reaction Boost, Finishing Plus, Negative Combo, Berserk Charge, Drive/Form/Summon Boost, MP Haste family, Draw, Jackpot, Lucky Lucky, element boosts, Defender, Item Boost, Hyper Healing, Drive Converter, Damage Control, and the SSTM constants each uses), then by the slot pass `FUN_1404014a0` with the growth families from `DAT_1405c9f00`.
5. **Auto-target** (`FUN_1403be4b0`): score = (distance + |dy| × height weight) × ((1 − cos) × 0.5 × (1 − angle weight) + angle weight), lowest wins, within the `plyr` record's range and Y-down band; `FUN_1403dccd0` holds the target for the `plyr` hold time.

## Implementation

- `player_sim::ptya`: record, selection, combo counters and gates; `player_sim::abilities`: the ability record and its SSTM rates; `player_sim::targeting`: the chooser and holder. Each has synthetic tests of the traced rules.
- `player_sim::lib`: `Clips::records` / `attacks` replace the fixed combo lists; `Action::Attack(i)` / `AirAttack(i)` index the PTYA record; `Player::try_attack` is the attack command; range trigger 50 is parsed; a hit or a guard clash sets `connected`.
- `test_room`: loads every distinct PTYA motion (and the weapon's), Sora's ability record is built from the equipped slots and rebuilt when the camp closes, the Shadow's TARGET shapes feed the chooser, and action events log the PTYA id, motion and counters.

## Evidence (private)

`research/playable-room-20261008/combo-01/autoplay-s1` (log and 81 captures). The three-press sequence on the arena Shadow chooses record 5 (motion 151), then 6 (motion 153), then the finisher 3 (motion 155), counters 1/1 → 2/1 → 3/1; hits ATKP 126, 128, 147 and 129 take the Shadow from 10 HP to 0.

## Checks

PASS `cargo fmt --all --check`; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS `cargo build --workspace --locked`; PASS release smoke (`--frames 30`) and the autoplay run above. NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

- Range-trigger end inclusivity (T055); `FUN_1403ba750`'s probe result (`probe_blocked` is false in the room).
- The test room has one enemy; the automatic target is it whenever targetable, and `targeting` is not yet driving the target.
- `ROOM_ABILITIES` is a test-room choice: the combo abilities start unequipped from the camp, with 48 extra AP.
- Counterguard (0x27) and the Square abilities are selected only through the chooser or the guard recoil; Upper Slash / Finishing Leap / Horizontal Slash on Square are not wired.

## Verdict / next

PROGRESS: the ground and air combo chain now follows the exe's selection. Next: damage and reactions (damage-02), then wire magic, items and Drive from the private specs.
