# T095 — Battle items: the Items submenu, the use motion and the heal

Owner Claude, 2026-10-10. Gates P18/P21 (test-room scope). Starts from T094. User request: rewrite the ability and combo system 1:1; this is stage 5, items.

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Private evidence: `research/playable-room-20261008/items-02/items-abilities-spec.md` Part A (Ghidra 12.1.4 on the read-only executable). Autoplay evidence is in `combo-01/autoplay-s7`.

## Findings (executable and installed data)

1. **Command**: a consumable's `item` record names a command (+0x10; execute 3, argument = the item id). The request plays motion 9 with an 8-tick blend and does not touch the combo counters (FUN_1403d6fc0). Items cannot be used in the air (FUN_1403b6cf0).
2. **Trigger**: motion 9's frame trigger 6 (tick 18) consumes the slot and queues the effect; the heal lands 30 ticks later whatever Sora does (FUN_1403f6790, FUN_1403f6960).
3. **Heal**: percent of max, not flat: HP += ceil(MaxHP × amount × (100 + Item Boost) / 10000); items 1, 2, 5 heal HP, 3 and 6 MP, 4 and 7 both, 0x83 full HP and MP. MP goes through the MP Charge rules, so an Ether during a charge shortens it by 12 per point (FUN_1403c4b80, FUN_1403c0890).
4. **Slots**: auto-reload items refill an emptied slot from the stock when the battle state drops (FUN_1403a1610).

## Implementation

- `player_sim`: `Action::Item`, `Player::use_item`, `take_item`, `Clip.item_use`, `Clips.item`.
- `test_room/item_room.rs`: item records, slots, the Items submenu list, the use request, the delayed heal and auto-reload; the Items row opens submenu 2, item shortcuts (Customize) use the slot holding their command, and the camp's Items screens show the live slots and stock. The room starts with a Potion and an Ether in the slots, taken from the stock.

## Checks

PASS fmt; PASS clippy (warnings denied); PASS `cargo test --workspace --locked`; PASS build; PASS release autoplay `autoplay-s7` (exit 0): a Potion at tick 1400 heals 8 (40% of 20) at 1446, an Ether at 1460 heals MP at 1506 through the charge rules. NOT RUN: interactive playtest; comparison with the installed game.

## Uncertainty

- Rows are not merged per command and show no count (the exe merges equal commands and shows a count).
- The heal effect (PAX group 0x2712) and the use voice are not drawn or played; the party-wide items act on Sora only.
- Item Boost's unit follows the SSTM field name; no Item Boost is equipped in the room.
- Auto-reload fires whenever the enemy cannot be hit, a stand-in for the battle state.

## Verdict / next

PROGRESS: items are used from the menu or a shortcut and heal by the exe's formula. Next: EXP and prizes, then Drive.
