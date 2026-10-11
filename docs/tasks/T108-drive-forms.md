# T108 — Drive Forms: the form gauge, the Antiform roll, form levels, the form actors and the growth abilities

Owner Claude, 2026-10-11. Gates P18/P21 (test-room scope). Starts from `main` at d0e9ee2, merged with `main` at 13ac0c9 (T107, Donald and Goofy). User request: Drive forms 1:1 (form actors, motions, movesets, leveling, growth abilities, Limit Form, Antiform, the party absorb).

## Prerequisites / source

Same selected source and private manifest (quarantined; full rehash at every launch). Ghidra 12.1.4 headless, read-only projects. Private evidence under `research/playable-room-20261008/`: `drive-01/drive-spec.md` (drive system, earlier), `growth-01/growth-spec.md` (growth abilities), `formcmd-01/formcmd-spec.md` (form commands, Wisdom's shots, Limit Form's limits, weapon motion sets), `drivefx-01/drivefx-spec.md` (transformation presentation, timing, party leave/return), `forms-data-01` / `forms-data-02` (form file extracts), and the unattended runs `drive-02/autoplay-0[1-4]` with their logs.

## Findings (executable and installed data)

1. **Form objects**: 03system `memt` row 0 names each form's object (Valor 0x55 `P_EX100_BTLF`, Wisdom 0x56 `_MAGF`, Limit 0x95d `_KH1F`, Master 0x57 `_TRIF`, Final 0x58 `_ULTF`, Anti 0x59 `_HTLF`); objentry +0x4e selects the PTYA, PRTY and WENT/WMST sets (2, 3, 66, 4, 5, 6).
2. **Weapons** (FUN_1403ef750): the Keyblade model comes from WENT, its motion set from 03system `wmst` row set × 2 + slot (`obj/%s.mset`): Valor `W_EX010_BTLF_R/_L`, Master `TRIF_R/_L`, Final `ULTF_R/_L`, Wisdom `MAGF`, Limit `KH1F`; Antiform's row is empty (no Keyblade). Slot 1 hangs on SKLT's second bone. The forms' attack volumes are range-10 triggers in those weapon sets.
3. **Change** (FUN_14040ce60, task FUN_14040d000): motion 0x34 (invulnerable, range 27), the scene light dims to 0x30/0x80 over 12 frames, 30 frames' wait, the form loads, the swap waits for motion 0x34 only while a party member is still in the field, HP/MP +6 a frame until the swap; the new object enters with motion 0x35, full HP/MP and level + 3 bars of 600 time. Revert has no wait or tint.
4. **Gauge** (FUN_1403c0b50, FUN_1403a92c0, FUN_1403a8800, FUN_1403a9cf0): time drains 1 a frame in battle (½ outside, × Form Boost), hits drain DriveDrain × 15 (× 0.2 on ZAKO), drive orbs refill × 12, Antiform turns refills into drains; at time 0 the form reverts once a command could run (FUN_140404a40).
5. **Roll** (FUN_140421d60): each Drive with both party slots usable adds to the anti counter; fmlv's Antiform rows give 0/10/25 % by it (100 with Light & Darkness); in battle a hit is Antiform, or a forced Final (3·3^n %, cap 75) while Final is not owned and its story flag is set.
6. **Levels** (FUN_1403e8380): fmlv thresholds, the cap is 2 + forms owned; a growth step swaps the family's item in the form's slots (level g) and in Sora's (level g − 1); EXP: Valor per hit landed, Wisdom per Heartless (1000..2011, not 1047..1059) defeated, Master per drive orb value, Final per Nobody defeated; Limit's (its finishers) is not wired.
7. **Growth abilities** (03system `pref/fmab`, FUN_1403a81f0, FUN_1403cb9f0, modes 0x10..0x12): High Jump 235/310/385/585 with motion 201; Aerial Dodge a new arc (145..300, 18..36 speed) once per airtime; Glide (Square above 60) with speed easing toward 16..32 and a sink capped per level; Dodge Roll (motion 209, 10..32 invulnerable frames, chains into Quick Run while Square is held at tick 10); Quick Run (30..38 speed, decay while held, 0..16 invulnerable frames).
8. **Commands per form** (FUN_1403b6cb0): the root command's +0x2c mask greys Magic in Valor, Limit and Antiform and Items in Antiform; Wisdom's Attack is "Shoot" (0x16d), its combo count always allowed, and frame trigger 1 fires the `wisdomform` bullet (ATKP 859, radius 50, 20 → 80 speed, homing 1.8°/tick to 100 units, 24 + 8 ticks).
9. **Party** (FUN_140400b20 / FUN_140400440): Valor takes Goofy, Wisdom Donald, Master/Final/Anti both, Limit none; members fade out over 15 frames and come back 200..300 units around Sora (eight directions from a rotating sector), facing him.

## Implementation

- `player_sim::drive`: the gauge's mode, bars and time, form drain, hits, gains, entry and refill. `player_sim::forms`: form records, party requirement, Drive availability, the roll, EXP and level-ups, EXP sources, the absorb rule and the return placement. `player_sim::growth`: fmab rows and Quick Run; `Player` gains High Jump, Aerial Dodge, Glide, Dodge Roll and Quick Run, the invulnerability timer, the drive-start / drive-in actions and Wisdom's Shoot gate. `player_sim::wisdom`: the bullets.
- `package_reader`: `battle_tables::form_levels` / `form_starting_abilities`, objentry character and form bytes, `motion::read_motion_ids_optional`.
- `test_room/kit.rs`: one kit loader for base Sora and every form (model, motion set with base fallback, both weapon motion sets, PTYA, PRTY, hit body). `test_room/drive_room.rs`: Drive / Revert rows and the forms submenu, the change task with on-demand loading on a worker thread, the kit swap and model respawn, the auto revert, EXP, the scene dimming, Wisdom's bullets, the party absorb and return. The HUD gauge draws the form frame, bar and digit.
- Test-room choices: Sora owns Valor, Wisdom, Limit, Master and Final at level 1 with nine Drive bars; F11 refills the gauge; F12 makes a stand-in party count when the room has none. The unattended run tries Limit, Valor and Wisdom between ticks 7480 and 8390.

## Checks

PASS `cargo fmt --all --check`; PASS `cargo clippy --workspace --all-targets --locked -- -D warnings`; PASS `cargo test --workspace --locked` (new: gauge time, Antiform bars, the roll, level-ups and growth swaps, the party rule and return spot, fmab rows, Quick Run, Glide, Aerial Dodge, High Jump, Dodge Roll, Wisdom bullets); PASS `cargo build --workspace --locked`; PASS smoke `cargo run -p windows_shell --locked -- --frames 180`; PASS `cargo build --release --features skate3 --locked -p windows_shell`; PASS release autoplay `drive-02/autoplay-04` (exit 0): Limit, Valor and Wisdom start, swap and revert; Valor shows two Keyblades and Goofy leaves, both return on revert; Limit hits with ATKP 126, Valor with 177/178, Wisdom's bullets (ATKP 859) defeat the Shadow. NOT RUN: interactive playtest; Master, Final and Antiform in a run; comparison with the installed game.

## Uncertainty

- Limit Form's limits (Sonic Rave, Last Arcanum, Strike Raid, Infinity) run on the form's BDX script and the limit system and are not ported; its Magic row stays greyed.
- The afterimage effect (DAT_142ae7c88), the drive particles (P_EX100 effects 1..27, form effects), voice 8, the ring-burst HUD rebind and the level-up panel are not drawn; their system sounds (114, 115, 132, 26) are queued.
- The members' 15-frame fade is not drawn (they disappear at its end); the return skips the line-of-sight ray; when a faded member stops counting as present is inferred.
- Quick Run's hand-back to mode 0 is inferred (a fall that lands at once); Dodge Roll moves by the motion's root curve [I]; "Square held" is request bit 2 [I]; Wisdom's muzzle is Sora's TARGET height, not effect n's position.
- The forced-Final story flag is treated as clear in the room; level-ups' messages are logged only.

## Verdict / next

PROGRESS: every form can be entered, plays its own model, Keyblades, motions and moves, drains and reverts by the exe's rules, and levels up; growth abilities work on base Sora and in the forms. Next: Limit Form's limits with the Limits work, the drive effects and afterimage, an interactive check of Master, Final and Antiform.
