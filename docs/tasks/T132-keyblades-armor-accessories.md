# T132 — Keyblades, armor and accessories: stat totals, models, form weapons

Owner Claude, 2026-10-16, requested by burba in the project thread. Gates P12/P18/P22 in the development room; no gameplay gate passes. Prerequisites: T120, T121. Build identity: `main` at `1e60690` plus this work, package 0.1.2, build 15194255 candidate (quarantined, session-validated at each launch).

## Steps and evidence (private root `%LOCALAPPDATA%/KH2FinalRewrite/research/gear-20261016/`)

1. Direct Ghidra 12.1.4 (read-only copy of the project): FUN_1403c1d90/FUN_1403c1190 (stat totals and seven resists), FUN_1403ef8b0 (WENT weapon lookup), FUN_14036afa0/14036b210 (weapon-change list), FUN_1403ed110 (No Experience, ability bit 116) in `stat-01`, `wchg-01`, `wobj-0*`, `noexp-0*`.
2. `equipment_probe` (source-bound session): every Keyblade/armor/accessory record, stat row, ability, picture and weapon object in `probe-02/equipment.json`. All 24 usable Keyblades and 3 Struggle weapons have their own model (12 bones, joint 25, shared motion) and 12 distinct attack-sphere layouts.
3. Rust: `player::equipment_stats`, `data::weapon_entry`, `test_room/gear.rs`, per-hand Keyblade models and spheres in `character_kit`/`drive_room`, Equip form-weapon rows, form weapon abilities in the Abilities list, No Experience gate, development stock of every Keyblade/armor/accessory with four armor and accessory slots.
4. `--gear-tour` drives the real camp controls through every item. `py/check_tour.py` recomputes each totals event from the probe alone.

## Checks

- PASS fmt, clippy, tests. Second full tour: 94 distinct equipment states, 0 failures (27 Keyblades, 34 armors, 33 accessories: Strength, Magic, Defense, AP, resists, weapon ability); 27 Keyblade kit rebuilds, 0 errors.
- NOT RUN: paired comparison against the installed game; per-item visual check of every model (captures retained, only a few viewed).

## Uncertainty / verdict / next

Still differing: per-Keyblade swing/hit sound bank (500xxx id remap, FUN_1403dadf0) is not ported; Keyblade-specific object effects (`W_EX010_*.a.us`) are not drawn; form-weapon stats count only while that form is active; slot counts and ownership are development choices; Donald/Goofy equipment untouched; ability consumers for Form Boost, Drive Boost, Item Boost not re-verified here.

Verdict: PROGRESS. Next: sound bank and weapon effects, paired captures.

## Follow-up (same day)

- Form Boost, Drive Boost and Item Boost: the Rust consumers match the earlier exe traces (Drive Boost +0.2 on drive gain, Form Boost ×0.8 on the form drain, Item Boost +50% on item heals). No change.
- Sound bank: the 27 Keyblade object entries are byte-identical except the object id (`probe-03`), so no per-Keyblade bank exists in the entries; nothing to port from them.
- Keyblade effects: every Keyblade has an `obj/<model>.a.us`. A kit built for a Keyblade now carries that file, and the swing trail and hit sparks use it after the swap; the trail length is recalibrated per Keyblade. Verified by a quick tour (swap and totals pass); visual comparison NOT RUN.
- Not done: Donald and Goofy equipment.
