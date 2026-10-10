# T092 — Magic: command menu, MP and MP Charge, the six spells

Owner Claude, 2026-10-10. Gates P18/P21 (test-room scope). Starts from T091 (`b036bf5`, merged with the movement branch at `bd797d9`). User request: rewrite the ability and combo system 1:1; this is stage 4, magic.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `research/playable-room-20261008/magic-01/magic-spec.md` (Ghidra 12.1.4 on the read-only executable; decompiles, the `magc` / `pref/magi` / `cmd` dumps and the six `.mag` script listings beside it). Autoplay evidence is in `combo-01/autoplay-s4` and `autoplay-s5` (the final build).

## Findings (executable and installed data)

1. **Chain**: menu or shortcut → command request → `FUN_1403d6fc0` dispatch (command Execute 2) → combo gate `FUN_1403d6ca0` → `FUN_1403c5170` magic start, which plays the cast motion from the `magc` record (ground, ground finisher, air) and runs the element's script trigger 20; the cast motion's frame trigger 17 calls `FUN_1403c6150`, which deducts MP (`FUN_1403d7e90` → `FUN_1403d3cf0`) and runs trigger 21.
2. **Spells are script programs** (one BDX program per element in the `.mag` files); the level/form variant is an integer, and the shape and timing come from the 03system `pref/magi` row. The room re-implements the six programs natively from the listings (spec §4), so the data stays in the installed files and the behaviour is the program's.
3. **MP and MP Charge**: cost is taken at release; a cast that would take MP below zero starts the charge instead (`FUN_1403d3cf0`, `FUN_1403c0890`, `FUN_1403c0de0`), charge length from the `prty` row (3000), MP gains of ×12 shorten it; commands are greyed from the menu colour rules (`FUN_140401d60`).
4. **Hits**: spell volumes are `ATTACK_MAGIC` volumes with ATKP ids (Fire 502, Blizzard 248, Thunder 249–251, Magnet 658, Cure 708/709, Reflect guard 573 with counters 574–578 / 667–669), through the same hit record, Interval slots and level rows as T091.
5. **Magnet** is reaction 5: the held object's physics modes 0xe/0xf (hold radius, spin from ATKP +0x10, pull 0.1 per unit clamped to ±20) with the TORNADO front/back motions.
6. **Cure** heals by the ATKP type 6 formula; Leaf Bracer (ability bit 114) keeps the cast from being interrupted.

## Implementation

- `package_reader::battle_tables`: `Attack` gains `spin`; `command_table::Command` carries the fields the menu needs.
- `player_sim::magic` (new): `Record`/`Row` readers for `magc` and `magi`, `Mp` (spend, charge, tick), the menu colour rules, `Spell` with the Fire / Blizzard / Thunder / Cure / Magnet / Reflect programs, `Volume` / `Strike` outputs, Leaf Bracer.
- `player_sim`: cast actions (ground, air, finisher), release at frame trigger 17, `Player.mp`, recoil; `combat`: `volume_hits_sphere` and `swept_volume` for moving volumes; `shadow`: `capture` / `release_held` and the held physics.
- `windows_shell`: command menu gains the Magic submenu (open, back, execute, greying), the gauge shows MP Charge (swapped groups 110/113/109/114, streak, fill by remaining/total), the room wires the L1 + face-button shortcuts, translucent placeholder volumes for the spells, and the autoplay cast schedule.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked` (all suites; 145 package_reader, 68 player_sim, 48 windows_shell lib, 8 kh2fm); PASS `cargo build --workspace --locked`; PASS `cargo run -p windows_shell --locked -- --frames 180`; PASS release autoplay `autoplay-s5` (exit 0): Fire (MP 100 → 88, ATKP 502 hits 3 each), Blizzard (88 → 73), Thunder (73 → 55, ATKP 250 hit 19), Magnet (55 → 25, enemy held, reaction `Held`), Reflect cast, Cure (MP 15 → 0 with the charge started, heal 42). NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

- The spell visuals (the `.mag` PAX effects) are not drawn; each live hit volume shows as a translucent shape tinted by element. Voices are not played.
- Reflect barrier lifetime (54 ticks) and Blizzard's start at the TARGET shape lift are inferred from the script listings, not observed (spec §9.2).
- Magnet's per-tick integration of the pull and when the hit re-arms are inferred (spec §9.3); a background hit is a stand-in.
- The MP-charge HUD is implemented from the exe's group swaps but was not observed on screen.
- Cure's party-target menu is skipped (single party), Magic Haste / Spice / Master Magic are not modelled (spec §9.10).

## Verdict / next

PROGRESS: the six spells cast, spend and charge MP, hit through the exe's record and damage rules, and the Magic submenu and shortcuts work in the room. Next: Sora's blow reactions and Aerial Recovery, EXP and prizes (stage 3 leftovers), then battle items, then Drive forms; spell visuals need the per-PAX texture atlas.
