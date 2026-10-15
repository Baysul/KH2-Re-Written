# T121 — Equipable Sora inventory, growth tiers and combo integration

Owner Codex, 2026-10-13. Gates P12/P18/P19/P22/P25 in the development room; no complete gameplay gate passes. User request: make the learned abilities available to equip, add missing growth abilities and make combo modifiers follow the original rules. Prerequisites: T120 fresh REA identity/native evidence, the existing camp and growth ports, and the user's instruction to leave work uncommitted. Build/source identity: checkout HEAD `3aa251e` plus uncommitted T120/T121 work, package 0.1.1; selected KH2FM Steam candidate still reports build 15194255/English and remains quarantined. Private source/engine identities remain in T120's `source-manifest-01.json` and native receipts.

## Steps and evidence

Private root: `%LOCALAPPDATA%/KH2FinalRewrite/research/equipable-abilities-20261013/`.

1. Trace the missing-menu entries to `ROOM_ABILITIES`, a fixed 35-item list containing no growth abilities. Replace it with a development inventory derived at load time from installed Sora PLRP templates, all three LVUP choice columns and leveling-form FMLV reward rows. The new bounded PLRP reader preserves all 32 raw slots/template and excludes other characters/form records. Template/choice multiplicity uses maxima; actual form rewards add to progression. Non-ability IDs are filtered, and exceeding the native 80 member slots is an error rather than silent truncation.
2. Keep one slot per growth family, at the highest tier in the installed normal-Sora template. This source produces LV3 High Jump, Quick Run, Dodge Roll, Aerial Dodge and Glide. Add F3 → Growth levels to choose a lower learned tier while preserving its equipped flag. Form-only MAX tiers are not granted as normal Sora slots. The camp still performs its native-style AP cost/availability checks; the development AP budget covers the granted inventory. This is immediate development availability, not campaign unlock progression.
3. Use the existing T120 REA evidence and separate source-byte-matched native listings. A further combo discrepancy is fixed: magic request type 2 passes `chain = true` to counter advancement independently of contact/Combo Master (`native-02/1403d6fc0.c`, instruction addresses `1403d71b6` and `1403d71bb`). Add combined synthetic tests exercising actual Player ground/air/cast sequences, finisher budgets, misses, MP-charge/Berserk behavior, negative signed limits and damage boosts.
4. Add a bounded `--ability-tour` using the actual camp controls: navigate to Sora's Abilities, equip each growth family and the combo modifiers, close the menu, and compare the resulting complete ability record with the requested loadout. Captures and event logs stay outside Git. The first shell launch did not retain a waited GUI process (`tour-01`); use an owned waited launch with an empty capture subdirectory (`tour-02`). Correct source-bound validation still runs on every launch.
5. `tour-02/events.jsonl` reports 64 distinct abilities in 69 occupied slots / 80, development AP 201. Eight complete captures are nonuniform. Visually inspect `captures/test-room-05.png` (all five growth rows equipped) and `test-room-06.png` (duplicate Combo Plus/Air Combo Plus, boosts, Finishing Plus, Negative Combo). Closing the camp produces growth `[3,3,3,3,3]`, ground/air limits 4/4 and two finishers; the complete-record comparison passes and the process exits 0. The old broad autoplay's Reflect/Cure script intruded after menu close, so this run does not qualify the intended growth jump; isolate those cast/item scripts from the bounded tour and repeat with the final release binary.
6. Rebuild with the existing `skate3` release feature and update `target/release/KH2RW Launcher.exe`. Preserve and hash-check the previous executable under `%LOCALAPPDATA%/KH2FinalRewrite/backups/T121-launcher/`; `launcher-receipt.json` records both identities and locations. The updated launcher and build output match SHA256 `2731a8692fd3587f6650c2fcc49a4d74e901d54483121a2f35d51fe9947aa266`. No commit, publication or version bump.
7. Run `tour-03` through that updated launcher executable with selected-source validation. `tour-03/events.jsonl` reports the same 64/69 inventory, equipped growth `[3,3,3,3,3]`, ground/air limits 4/4, two finishers, boosts 10/10/50 and complete-record verdict `pass: true`; process exit 0. All eight captures are nonuniform. Visually inspect `captures/test-room-05.png` (growth rows), `test-room-06.png` (stacked modifiers) and `test-room-07.png` (airborne High Jump). HighJump motion 201 starts at tick 1108 and the last capture reports HighJump at tick 1148. The tour contains no scripted spell cast. These are rewrite integration observations, not a paired reference comparison.

## Implementation

- `package_reader::battle_tables::member_ability_lists`: bounded member-template slot decoding.
- `test_room/ability_inventory.rs`: source-derived development inventory, duplicate counts, growth-tier selection and bounded menu tour.
- `test_room.rs`: remove the fixed ability list, seed persistent member slots from the inventory, provide the AP budget, expose menu/battle-state evidence and tour validation.
- `debug_menu.rs`: Growth levels page and left/right tier selection.
- `player_sim::Player::cast`: original unconditional combo advancement for magic requests, with combined regressions.

## Checks

PASS scoped inventory tests and 124 player_sim tests. Final required checks:

- PASS `cargo fmt --all --check` (`fmt-final.log`).
- PASS `cargo clippy --workspace --all-targets --locked -- -D warnings` (`clippy-final-03.log`).
- PASS `cargo test --workspace --locked`: 488 tests, zero failures (`tests-final-03.log`).
- PASS `cargo build --workspace --locked` (`build-final.log`).
- PASS `cargo run -p windows_shell --locked -- --frames 180`: exit 0 (`smoke-final.log`); default controller mapping warning retained.
- PASS release build with `--features skate3` (`release-build-final.log`), delivered-launcher hash equality and preserved backup (`launcher-receipt.json`).
- PASS final actual-menu tour, complete equipped-record check and visual capture inspection (`tour-03`).

Early compile/Clippy errors are retained in `game-build-01.log` and `clippy-final.log`, corrected before accepted checks. Final installed-game paired comparison NOT RUN. Provenance review PASS: new repository artifacts are Rust, Java and Markdown; original tables, disassembly, manifests, captures, logs and executable backup remain outside the checkout. No source qualification or final gameplay gate promotion.

## Uncertainty / verdict / next

Granting the development inventory and its AP budget is an explicit development choice. Normal campaign/save acquisition, party ability menus and unsupported consumers such as summons/Hyper Healing/Combination Boost remain open. Static branch agreement and the actual rewrite-menu tour do not establish reference timing, window inclusivity, movement/pose/root/collision equivalence or whole-system 1:1 fidelity. Installed files and user saves remain unchanged; research/captures stay private.

Verdict: PROGRESS. Next: paired installed/rewrite scenarios for growth, modifier combinations and form transitions, plus remaining action/support consumers and campaign progression.
