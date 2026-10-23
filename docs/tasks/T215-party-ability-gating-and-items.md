# T215 — Party ability gating, own item slots and Sora's party items

Owner Claude, 2026-10-23, requested by burba in the project chat ("keep working on it until
there's nothing left to do", "keep going"), handed over by the project's coordinator session;
issue #77. Gate: development of the test room only; no final gameplay acceptance is inferred.
Prerequisites: T136 (party spells), T186 (party hit, KO, items), T207 (Sora's party items), T210.
Build identity: branch `claude/project-thread-ww9y8z` from `main` at `b5f4230`, package 0.1.2,
Rust 1.99.0 Windows MSVC, release built with `--features skate3`. Source: the selected Steam
KH2FM build, validated each run; the install was only read.

Private evidence root: `%LOCALAPPDATA%/KH2FinalRewrite/research/party-1to1-20261023/`
(`party-gating-spec.md`; headless Ghidra 12.1.4 on a read-only copy of the analysed project:
`out/g1`..`out/g8`, `out/e6`); unattended runs in the private `kh2-party-1to1/runs/pit1`..`pit5`,
`runs/g1`, `runs/g2`.

## What the executable does

- `FUN_1401b2880(obj, entry, 1, 1)` gates every action entry of Donald's and Goofy's tables. An
  entry with an ability (+0x30, the ability bit; `FUN_1401cccf0` fills +0x34 with its ability
  item from the map at 0x1407249d0 and +0x38 with `FUN_1401dc060(item)`) is refused when the
  member's frequency byte `R[0xf5 + idx]` is 3, when the item is in none of the record's 80
  ability slots (+0x54, low 15 bits) or when no such slot has bit 15 (equipped). Attacks have
  no ability. Callers: `FUN_1401cc650` (spells and specials; Cure through `FUN_1401cc1a0`) and
  `FUN_1401cd0a0` (attacks).
- The map: Donald Fire 0xa5, Blizzard 0xa6, Thunder 0xa7, Cure 0xa8 (index 0..3); Goofy Tornado
  0x1a7, Bash 0x1ad, Turbo 0x1a9 (index 0..2). The frequency index is 7 + the index; items 1..7
  use 0..6.
- The selection loop `FUN_1401b1d40`: pass 0 skips Rare (2) entries and a Well-Balanced (1)
  entry with a cost above the MP; a pick that repeats the last three commands goes to pass 1,
  which tries only the Rare entries; none there, or the repeat again, and the member follows.
- The character record is `save + 0x24f0 + 0x114 * (t - 1)` (`FUN_1403a14e0`): item slots +0x34
  (count +0x12), ability slots +0x54, battle style +0xf4, frequencies +0xf5..+0x113.
- A new game zeroes the save (style and every frequency 0) and applies the plrp rows of key 0
  only (`FUN_1403e4530(0)`): Donald 2 item slots (Potion, Ether), Thunder and Cure equipped;
  Goofy 3 item slots (three Potions), Goofy Bash and Item Boost equipped. No code applies the
  key-1 rows.
- A member's item entries (`FUN_1401cc970` HP, `FUN_1401cce90` MP) take an item while its
  frequency is below 2 or a partner is in need, and count it in the member's own item slots
  (`FUN_1403c4380(item, char)`; only char 100 reads the inventory). The use removes it from the
  same slots (`FUN_1403c49c0(item, char)`, the last slot holding it).

## What the room does now

- `party_members::Save` mirrors the record (ability slots with the equip bit, item slots,
  frequencies); each member starts from its plrp key-0 row (`battle_tables::new_game_row`, the
  03system `item` type 0 ids into the item slots).
- Spells and specials need their ability equipped and not off; the selection loop applies Free,
  Well-Balanced and Rare and the follow-on-repeat rule.
- Item entries draw on the member's own slots, with the item frequency rule, and the use spends
  from them (Sora's item slots are no longer touched by the party).
- A member's item heals with its own Item Boost (its equipped Item Boosts × SSTM +0x164).
- `--party-items-tour <dir>`: Sora uses a Mega-Potion, a Mega-Ether and a Megalixir after
  Donald and Goofy are worn down (Goofy knocked out before the Megalixir).

## Checks

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| `cargo test --workspace --locked` | PASS (one test ignored as before; new: the new-game record and the gate, the frequencies in the selection loop, the plrp new-game row, the tour's schedule) |
| `cargo build --workspace --locked` | PASS |
| Smoke `cargo run -p main --locked -- --frames 180` | PASS |
| Release `build-release-skate3.bat` (private target dir) | PASS; it leaves `KH2RW Launcher.exe` and the two viewers |
| Unattended `--party-items-tour` on `KH2RW Launcher.exe` (private `runs/pit5`) | exit 0; the Mega-Potion heals Sora, Donald and Goofy (HP), the Mega-Ether all three (MP), the Megalixir all three and revives the downed Goofy (`was_down`); Donald uses his own Ether and Potion on himself |
| Unattended `--autoplay` with party combat on `KH2RW Launcher.exe` (private `runs/g2`) | exit 0; Donald casts Thunder only (8 casts; Fire and Blizzard are not his on a new game), Goofy's Bash runs, Donald's Ether comes from his own slots |

## Uncertainty

- [I] The repeat test's obj+0xe08 < 0xe condition is not modelled (the repeat rule always applies).
- [I] A member's Item Boost is taken as the equipped Item Boosts × SSTM +0x164, as Sora's.
- [I] The emergency passes (`FUN_1401b3a30`) apply the gate but not the frequency rules.
- Not done: the camp's Customize and Abilities screens for Donald and Goofy (the room's record
  cannot be edited yet; it is the new-game one); who could write frequency 3; which scripts use
  the ability syscalls `FUN_140536a50` / `FUN_14041e7e0`.
- No comparison against the installed game's frames or timings; no owner playtest.

## Verdict

PARTIAL. Donald's and Goofy's spells and specials are gated by their equipped abilities and
Customize frequencies, their items come from and are spent from their own slots, and Sora's
Mega-Potion, Mega-Ether and Megalixir heal the party in a tour.

## Next task

The camp's Customize and Abilities screens for the party; Limit Form's lens-flare effect
programs (188, 189, 191) and Final Form's world-matrix program (99).
