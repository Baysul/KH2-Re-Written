# KH2FM abilities, growth and combo modifiers

T120 records a fresh inspection of the selected Steam executable and seven installed tables. These findings describe static code and data. Installed gameplay comparison remains NOT RUN; a complete 1:1 recreation is not established. Original bytes, pseudocode, hashes and file offsets are retained privately under `%LOCALAPPDATA%/KH2FinalRewrite/research/abilities-20261013/`.

## How abilities become battle state

An ability item ID and its ability bit are different identifiers. The item record's +4 field supplies the bit. An owned slot contributes only when its high bit is set; the lower 15 bits identify the item. AP controls equipping in the menu; the battle pass consumes equipped slots rather than charging AP again.

The rebuild starts with an empty bit set, ground/air limits of three and one finisher. Equipment contributes first, in order: member weapons, the linked form weapon, armor, accessories. Weapons select their stat row through item +6; armor/accessories through +4. Stat-row +2 grants an ability. Each equipment occurrence and equipped slot contributes separately, so duplicate support modifiers stack even though their presence bit is set only once.

The normal member has 80 ability slots and a linked form has 24. With a form linked, normal slots whose item byte +6 is nonzero are skipped. This is the action/growth versus support discriminator; calling it only a growth flag would obscure the action abilities also replaced by a form. The form's equipped slots are then added.

| Finding | Native address | Private evidence | Rust path |
|---|---|---|---|
| Reset values | `1404016b0` | `native-01/1404016b0.c` | `player::abilities::Abilities::default` |
| Ability-specific adders and presence bit | `140401730` | `native-01/140401730.c` | `Abilities::add` |
| Member/form slots and growth scan | `1404014a0` | `native-01/1404014a0.c` | `Abilities::equip` |
| Equipment order and rebuild | `1403c1d90`, `1403c1190` | corresponding `native-01/*.c` | room `AbilitySource::build_with_form` |
| Category chooses the stat key | `1403e3ef0` | `native-01/1403e3ef0.c` | `data::item_table::Item::equipment_stats_id` |

The room now rebuilds from current equipment instead of retaining the Kingdom Key's ability after a weapon change. Both member weapon slots, the form weapon, armor and accessory abilities participate in the same pass. The development room leaves the second member weapon empty; its save and menu population are unqualified. This does not establish complete equipment/stat fidelity.

## Growth abilities

The five families are High Jump, Quick Run, Aerial Dodge, Glide and Dodge Roll. Growth does not add levels together: a descending five-entry bit scan selects the highest present level. A direct PE file read of the complete 25-byte growth lookup matches the existing Rust lookup. Each family's fifth entry is zero; the original code scans that entry literally. The port retains that edge behavior rather than inventing a clamp.

`pref/fmab` contains five parameter rows of 17 float words each. Level selection uses a one-based lookup, with row zero as the fallback for a missing/out-of-range row. The Rust reader now rejects a malformed nonzero pointer or truncated row instead of silently treating corrupt data as an absent row. That is importer validation; native behavior on malformed source is not claimed.

| Family | Parameters and behavior | Evidence |
|---|---|---|
| High Jump | Replaces jump height and selects its growth motion when the player/object gates allow it | `native-01/1403b59b0.c` |
| Quick Run | Initial velocity comes from direction × row speed; physics decays velocity before movement. Holding runs down a timer; release/expiry changes decay and motion | `native-01/1403a9830.c`, `140423480.c`, `140423500.c` |
| Aerial Dodge | Uses its own height/speed and the shared airborne mode; mode entry precedes the new request flag, so the normal jump-release cut is not armed for this rise | `native-02/1403cb9f0.c`, `native-04/1403cbd30.c`, `native-05/140409ad0.c` |
| Glide | Row controls speed, steering and descent. Descent is capped and falls to zero near the row's floor height | `native-01/1404237a0.c`, `native-02/140423a70.c` |
| Dodge Roll | Uses its motion and row invulnerability; Square dispatch decides its availability and priority | `native-02/1403a81f0.c` |

Growth rewards come from form progression, not from using the normal growth ability in isolation. The form-level table supplies EXP, a growth step and a reward ability. On a growth increase, the original replaces the form's growth slot and the preceding tier in Sora's slots while preserving equipped flags. Its cap is two plus the number of owned forms among the five leveling forms. These branches match the existing `Forms::add_exp` implementation (`native-02/1403e8380.c`). The room now retains Sora's 80 slots even without the camp UI, so rewards survive subsequent ability rebuilds.

`140401680` also has a special context branch returning level five. `native-03/1403abcd0.c` shows world/area bytes and a global flag gate; its scenario meaning and activation are unqualified. The normal Rust player accessor does not model this special context.

## Combo modifiers

Attack selection uses PTYA records, not a fixed list of three animations. Record gates check motion presence, required ability bit, ground/air and finisher status. Target geometry, record priority and alternates then determine the selected motion. The item that enables a move does not by itself guarantee that move is selected.

| Modifier | Traced effect |
|---|---|
| Combo Plus | Adds one to the signed ground combo limit per occurrence |
| Air Combo Plus | Adds one to the signed air combo limit per occurrence |
| Negative Combo | Subtracts one from both limits; native byte wrapping is retained |
| Finishing Plus | Adds one to the allowed finisher chain count |
| Berserk Charge | Prevents the normal finisher-limit tests from succeeding while MP charges; its strength bonus is consumed separately |
| Combo Master | Allows the main attack command to chain after a miss; Square combo moves still require contact |
| Combo/Air Combo Boost | Adds damage using the appropriate capped combo count and accumulated boost rate |
| Reaction Boost | Adds its damage percentage in the reaction branch |

The combo step and finisher step are separate counters, capped at 100. An actionable fresh attack starts at 1/1. A connected chain inside animation range 11 advances the combo step; a chain inside range 50 also advances the finisher step when permitted by Finishing Plus. Finisher gates compare the signed ground/air limit with the step plus the PTYA record's signed offset. An active finisher window can authorize a finisher directly. Berserk Charge with MP charging suppresses that authorization.

The damage modifier branch tests ability presence bits and attack-kind flags, in ground → air → reaction order. It does not select a branch merely because its accumulated rate is positive. For a combo boost, let `n = min(combo_step, signed_limit)`, `p` be the accumulated percentage and `d` the damage entering this stage. The native calculation is `d + trunc((n × p × d + 99) / 100)`, with 32-bit wrapping products/additions. Reaction Boost omits `n`. Integer division and rounding order are observable parts of this algorithm. A wrapped-zero ground boost still suppresses later matching branches when its bit is present.

Evidence: `native-01/1403d6ca0.c`, `1403d7780.c`, `1403d7850.c`, `140402750.c`; `native-02/1403d6fc0.c`, `1403a9950.c`, `1403c12f0.c`; `native-03/1403d74a0.c`, `1403d7480.c`. Reports beside these files retain instruction addresses, original file offsets and exact-byte comparisons. Rust implements the gates in `player::ptya`, command routing in `Player`, and damage terms in `combat`.

## Evidence boundary and remaining work

REA 6.4.0 statically identified the installed file as mixed CLR/native. Its canonical Evidence ID is `ev_ad0286c10649ad1fe055bca47488521179e02f0ecf80142e4e721c2cf5aac4f5` (`rea-managed-01.json`). The REA Windows Ghidra provider rejected native analysis of that target class (`rea-equip-01.json`). The native findings above were produced separately by Ghidra 12.1.4 through the project's read-only research route; they are not REA native dossiers. No target execution or configuration repair was used to bypass that limitation.

Complete raw table receipts are in `tables-02/tables.json`: item, SSTM, FMAB, PTYA, FMLV, LVUP and PLRP. This final probe uses uncached full-source verification before and after reading. `tables-01` retains the earlier session binding, whose closing verification reused its initial admission. Source validation remains quarantined, without vendor authenticity or complete edition qualification. Old T090/T108 private paths were absent during this audit; current conclusions use the fresh evidence locations above.

Static branches and synthetic regressions do not prove timing, animation-window inclusivity, installed poses/root motion, collision probes, camera response or recorded gameplay agreement. Hyper Healing and Combination Boost currently accumulate their parameters without implemented gameplay consumers; summon behavior is also incomplete. Special-context growth, equipment/save population, full progression/save integration, every action/support consumer and reference comparisons remain in the denominator. P18/P19/P22 and the full 1:1 request therefore remain open.
